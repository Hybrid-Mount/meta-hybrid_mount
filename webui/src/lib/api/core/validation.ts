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

import { z } from "zod/v4";

// --- Daemon JSON response envelope ---

export const daemonErrorEnvelopeSchema = z.object({
  type: z.literal("error"),
  error: z.string(),
});

export const daemonFailureEnvelopeSchema = z.object({
  ok: z.literal(false),
  error: z.string(),
});

export const daemonSuccessEnvelopeSchema = z.object({
  ok: z.literal(true),
  data: z.unknown().optional(),
});

// A daemon command response may be either the error envelope or the success envelope,
// or a raw payload. We validate the common patterns.
export const daemonResponseSchema = z.union([
  daemonErrorEnvelopeSchema,
  daemonFailureEnvelopeSchema,
  daemonSuccessEnvelopeSchema,
]);

// --- WebUI session (daemon webui-start response) ---

export const webuiSessionSchema = z.object({
  base_url: z.string().min(1),
  token: z.string().min(1),
});

export type WebuiSession = z.infer<typeof webuiSessionSchema>;

// --- Structured error extraction ---

export function extractStructuredError(payload: unknown): string | null {
  const parsed = daemonErrorEnvelopeSchema.safeParse(payload);
  if (parsed.success) return parsed.data.error;

  const failure = daemonFailureEnvelopeSchema.safeParse(payload);
  if (failure.success) return failure.data.error;

  return null;
}

// --- Generic JSON output parser ---

export function parseDaemonJson(raw: string): unknown {
  let parsed: unknown;
  try {
    parsed = JSON.parse(raw);
  } catch (cause) {
    throw new Error(
      `Failed to parse daemon JSON: ${cause instanceof Error ? cause.message : cause}`,
    );
  }

  const structured = extractStructuredError(parsed);
  if (structured) throw new Error(structured);

  // Unwrap the { ok: true, data: ... } envelope when present
  const envelope = daemonSuccessEnvelopeSchema.safeParse(parsed);
  if (envelope.success) return envelope.data.data;

  return parsed;
}
