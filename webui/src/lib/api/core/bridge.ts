/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { APP_VERSION } from "../../constants_gen";
import { AppError } from "./error";
import { shellEscapeDoubleQuoted } from "./shell";
import {
  parseDaemonJson,
  webuiSessionSchema,
  type WebuiSession,
} from "./validation";

interface KsuExecResult {
  errno: number;
  stdout: string;
  stderr: string;
}

interface KsuModule {
  exec: (cmd: string, options?: unknown) => Promise<KsuExecResult>;
}

// Discriminated union matching Rust DaemonCommand sub-enums.
// Wire format is flat: {"type": "kebab-case-name"}.
// Organized by domain group to match the Rust SystemCommand / ConfigCommand /
// ModulesCommand / BatchCommand sub-enums.
export type DaemonCommandPayload =
  // ── SystemCommand: health, lifecycle, storage, info ──
  | { type: "ping" }
  | { type: "webui-start" }
  | { type: "shutdown" }
  | { type: "init" }
  | { type: "status" }
  | { type: "api-storage" }
  | { type: "api-mount-stats" }
  | { type: "api-mount-topology" }
  | { type: "api-partitions" }
  | { type: "api-system-info" }
  | { type: "api-version" }
  | { type: "api-open-url"; url: string }
  | { type: "api-reboot" }
  | { type: "clear-mount-errors" }
  // ── ConfigCommand: config CRUD ──
  | { type: "api-config-get" }
  | { type: "api-config-set"; config: unknown }
  | { type: "api-config-patch"; patch: unknown; apply_runtime?: boolean }
  | { type: "api-config-reset" }
  // ── ModulesCommand: module operations ──
  | { type: "api-modules-list"; path?: string | null }
  | { type: "api-modules-apply"; modules: unknown[] }
  // ── BatchCommand ──
  | { type: "batch"; commands: DaemonCommandPayload[] };

let ksuExec: KsuModule["exec"] | null = null;

interface MockModeEnv {
  MODE?: string;
  DEV?: boolean;
  VITE_USE_MOCK?: string;
}

function hasKsuBridge(): boolean {
  const bridge = (globalThis as { ksu?: unknown }).ksu;
  return typeof bridge === "object" && bridge !== null && "exec" in bridge;
}

if (hasKsuBridge()) {
  try {
    const ksu = await import("kernelsu").catch(() => null);
    ksuExec = ksu ? ksu.exec : null;
  } catch {
    console.debug("ksu bridge init skipped");
  }
}

export function resolveShouldUseMock(env: MockModeEnv): boolean {
  const override = env.VITE_USE_MOCK?.trim().toLowerCase();
  if (override === "false" || override === "0" || override === "off") {
    return false;
  }
  if (override === "true" || override === "1" || override === "on") {
    return true;
  }
  return Boolean(env.DEV) || env.MODE === "test";
}

export const shouldUseMock = resolveShouldUseMock(import.meta.env);
export const defaultVersion = APP_VERSION;
export const hasExecBridge = Boolean(ksuExec);
const DAEMON_WAKE_TIMEOUT_MS = 5000;
const DAEMON_HTTP_TIMEOUT_MS = 30000;
const DAEMON_MODULES_TIMEOUT_MS = 15000;

const SESSION_STORAGE_KEY = "mhm_webui_session";
const DAEMON_PING_TIMEOUT_MS = 750;
const SSE_RECONNECT_DELAY_MS = 1000;

let daemonReady: Promise<void> | null = null;
let webuiSession: WebuiSession | null = null;
let sseSource: EventSource | null = null;
let sseSourceUrl: string | null = null;
let sseReconnectTimer: number | null = null;
type SseStateHandler = (state: unknown) => void;
let sseHandlers: SseStateHandler[] = [];

function loadStoredSession(): WebuiSession | null {
  try {
    const raw = sessionStorage.getItem(SESSION_STORAGE_KEY);
    if (!raw) return null;
    const parsed = webuiSessionSchema.safeParse(JSON.parse(raw));
    return parsed.success ? parsed.data : null;
  } catch {
    return null;
  }
}

function persistSession(session: WebuiSession): void {
  try {
    const raw = JSON.stringify(session);
    sessionStorage.setItem(SESSION_STORAGE_KEY, raw);
  } catch {
    /* storage unavailable */
  }
}

function clearStoredSession(): void {
  try {
    sessionStorage.removeItem(SESSION_STORAGE_KEY);
  } catch {
    /* storage unavailable */
  }
}

export function buildSseUrl(session: WebuiSession): string {
  return `${session.base_url}/events?token=${encodeURIComponent(session.token)}`;
}

function clearPendingSseReconnect(): void {
  if (sseReconnectTimer !== null) {
    window.clearTimeout(sseReconnectTimer);
    sseReconnectTimer = null;
  }
}

function scheduleSseReconnect(): void {
  if (sseReconnectTimer !== null) return;
  if (!webuiSession || sseHandlers.length === 0) return;

  sseReconnectTimer = window.setTimeout(() => {
    sseReconnectTimer = null;
    startSse();
  }, SSE_RECONNECT_DELAY_MS);
}

function setWebuiSession(session: WebuiSession): void {
  webuiSession = session;
  startSse();
}

function clearWebuiSession(): void {
  webuiSession = null;
  stopSse();
}

async function pingDaemonHttp(session: WebuiSession): Promise<boolean> {
  const controller = new AbortController();
  const timer = window.setTimeout(
    () => controller.abort(),
    DAEMON_PING_TIMEOUT_MS,
  );
  try {
    const response = await fetch(`${session.base_url}/rpc`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify({ command: { type: "ping" } }),
      signal: controller.signal,
    });
    return response.ok;
  } catch {
    return false;
  } finally {
    window.clearTimeout(timer);
  }
}

function requireExec(): KsuModule["exec"] {
  if (!ksuExec) throw new AppError("No KSU environment");
  return ksuExec;
}

async function runCommand(command: string): Promise<KsuExecResult> {
  const exec = requireExec();
  return exec(command);
}

async function runCommandExpectOk(command: string): Promise<string> {
  const { errno, stdout, stderr } = await runCommand(command);
  if (errno === 0) return stdout;
  throw new AppError(stderr || `command failed: ${command}`, errno);
}

function hybridMountCommand(binaryPath: string, args: string): string {
  return `"${shellEscapeDoubleQuoted(binaryPath)}" ${args}`;
}

function withTimeout<T>(
  promise: Promise<T>,
  ms: number,
  message: string,
): Promise<T> {
  return new Promise((resolve, reject) => {
    const timer = window.setTimeout(() => reject(new AppError(message)), ms);
    promise.then(
      (value) => {
        window.clearTimeout(timer);
        resolve(value);
      },
      (error) => {
        window.clearTimeout(timer);
        reject(error);
      },
    );
  });
}

export async function readModuleProp(modulePath: string): Promise<string> {
  return runCommandExpectOk(
    `cat "${shellEscapeDoubleQuoted(modulePath)}/module.prop"`,
  );
}

const MISSING_MODULE_PROP_SENTINEL = "__HYBRID_MOUNT_MODULE_PROP_MISSING__";

export async function readModulePropIfPresent(
  modulePath: string,
): Promise<string | null> {
  const escapedPath = shellEscapeDoubleQuoted(modulePath);
  const output = await runCommandExpectOk(
    `if [ -f "${escapedPath}/module.prop" ]; then cat "${escapedPath}/module.prop"; else printf '%s' '${MISSING_MODULE_PROP_SENTINEL}'; fi`,
  );
  return output === MISSING_MODULE_PROP_SENTINEL ? null : output;
}

async function coldStartDaemon(binaryPath: string): Promise<WebuiSession> {
  const raw = await withTimeout(
    runCommandExpectOk(hybridMountCommand(binaryPath, "daemon webui-start")),
    DAEMON_WAKE_TIMEOUT_MS,
    "hybrid-mount daemon wake timed out",
  );
  const rawPayload = parseDaemonJson(raw);
  const parsed = webuiSessionSchema.safeParse(rawPayload);
  if (!parsed.success) {
    throw new AppError("hybrid-mount daemon returned invalid WebUI session");
  }
  return parsed.data;
}

export async function ensureDaemonAwake(binaryPath: string): Promise<void> {
  if (shouldUseMock || !hasExecBridge) return;
  if (!daemonReady) {
    daemonReady = (async () => {
      const stored = loadStoredSession();
      if (stored && (await pingDaemonHttp(stored))) {
        setWebuiSession(stored);
        return;
      }
      clearStoredSession();

      const session = await coldStartDaemon(binaryPath);
      setWebuiSession(session);
      persistSession(session);
    })().catch((error) => {
      daemonReady = null;
      clearWebuiSession();
      clearStoredSession();
      throw error;
    });
  }
  return daemonReady;
}

export function parseDaemonJsonOutput(raw: string): unknown {
  try {
    return parseDaemonJson(raw);
  } catch (cause) {
    throw new AppError(
      cause instanceof Error
        ? cause.message
        : "Failed to parse daemon JSON output",
    );
  }
}

async function runDaemonHttp(
  session: WebuiSession,
  command: DaemonCommandPayload,
): Promise<unknown> {
  const controller = new AbortController();
  const timeoutMs =
    command.type === "api-modules-list"
      ? DAEMON_MODULES_TIMEOUT_MS
      : DAEMON_HTTP_TIMEOUT_MS;
  const timer = window.setTimeout(() => controller.abort(), timeoutMs);
  let response: Response;
  let text: string;

  try {
    response = await fetch(`${session.base_url}/rpc`, {
      method: "POST",
      headers: {
        "content-type": "application/json",
        authorization: `Bearer ${session.token}`,
      },
      body: JSON.stringify({ command }),
      signal: controller.signal,
    });
    text = await response.text();
  } catch (error) {
    if (controller.signal.aborted) {
      throw new AppError(`daemon HTTP request timed out after ${timeoutMs}ms`);
    }
    throw error;
  } finally {
    window.clearTimeout(timer);
  }

  let payload: unknown;
  try {
    payload = parseDaemonJsonOutput(text);
  } catch (e) {
    if (!response.ok) {
      throw e instanceof AppError
        ? e
        : new AppError(`daemon HTTP request failed: ${response.status}`);
    }
    throw e;
  }
  if (!response.ok) {
    throw new AppError(`daemon HTTP request failed: ${response.status}`);
  }
  return payload;
}

// In-flight request deduplication: identical concurrent commands share one promise.
const inflightRequests = new Map<string, Promise<unknown>>();

export async function runDaemonCommand(
  command: DaemonCommandPayload,
  binaryPath: string,
): Promise<unknown> {
  const key = JSON.stringify(command);
  const existing = inflightRequests.get(key);
  if (existing) return existing;

  const promise = runDaemonCommandInternal(command, binaryPath);
  inflightRequests.set(key, promise);
  try {
    return await promise;
  } finally {
    inflightRequests.delete(key);
  }
}

async function runDaemonCommandInternal(
  command: DaemonCommandPayload,
  binaryPath: string,
): Promise<unknown> {
  await ensureDaemonAwake(binaryPath);
  let lastError: unknown = null;
  let firstError: unknown = null;

  for (let attempt = 0; attempt < 2; attempt += 1) {
    const session = webuiSession;
    if (!session) break;

    try {
      return await runDaemonHttp(session, command);
    } catch (error) {
      lastError = error;
      if (attempt === 0) firstError = error;
      if (
        error instanceof AppError &&
        error.message.includes("daemon HTTP request timed out")
      ) {
        throw error;
      }
      console.debug("daemon HTTP bridge request failed", error);
      daemonReady = null;
      clearWebuiSession();

      if (attempt === 0) {
        await ensureDaemonAwake(binaryPath);
        continue;
      }
    }
  }

  if (lastError) {
    if (firstError && firstError !== lastError) {
      console.debug("original daemon error (retry also failed)", firstError);
    }
    throw lastError;
  }
  throw new AppError("hybrid-mount daemon WebUI session is unavailable");
}

export function onSseStateUpdate(handler: SseStateHandler): () => void {
  sseHandlers.push(handler);
  startSse();
  return () => {
    sseHandlers = sseHandlers.filter((h) => h !== handler);
    if (sseHandlers.length === 0) {
      stopSse();
    }
  };
}

export function startSse(): void {
  if (shouldUseMock || !hasExecBridge) return;
  if (sseHandlers.length === 0) return;
  const session = webuiSession;
  if (!session) return;

  const url = buildSseUrl(session);
  if (sseSource) {
    if (sseSourceUrl === url) return;
    stopSse();
  }

  clearPendingSseReconnect();

  const source = new EventSource(url);
  sseSource = source;
  sseSourceUrl = url;

  source.addEventListener("state_update", (event: MessageEvent) => {
    try {
      const state = JSON.parse(event.data as string) as unknown;
      for (const handler of sseHandlers) {
        try {
          handler(state);
        } catch (e) {
          console.error("SSE handler error", e);
        }
      }
    } catch (e) {
      console.error("Failed to parse SSE state update", e);
    }
  });

  source.onerror = () => {
    console.debug("SSE connection error, scheduling reconnect");
    source.close();
    if (sseSource === source) {
      sseSource = null;
      sseSourceUrl = null;
      scheduleSseReconnect();
    }
  };
}

export function stopSse(): void {
  clearPendingSseReconnect();
  if (sseSource) {
    sseSource.close();
    sseSource = null;
  }
  sseSourceUrl = null;
}
