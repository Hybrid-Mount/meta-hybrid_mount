import { runDaemonCommand } from "../core/bridge";
import { DEFAULT_CONFIG, PATHS } from "../../constants";
import type {
  KernelUnameValues,
  KasumiStatus,
  KasumiUnameConfig,
} from "../../types";
import { patchConfigFile } from "../repos/configRepo";
import { buildKasumiStatusFromPayload } from "../codec/runtimeCodec";
import { AppError } from "../core/error";
import { isRecord, isString } from "../core/guards";

async function applyKasumiRuntimeConfig(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-apply-config-runtime" }, PATHS.BINARY);
}

async function updateKasumiConfig(
  patch: Record<string, unknown>,
  options: { applyRuntime?: boolean } = {},
): Promise<void> {
  await patchConfigFile(
    { kasumi: patch },
    { applyRuntime: options.applyRuntime !== false },
  );
}

export async function getKasumiStatus(): Promise<KasumiStatus> {
  const payload = await runDaemonCommand(
    { type: "kasumi-status" },
    PATHS.BINARY,
  );
  const status = buildKasumiStatusFromPayload(
    payload,
    DEFAULT_CONFIG.kasumi,
    {},
  );
  if (status) return status;
  throw new AppError("kasumi status returned invalid payload");
}

export async function setKasumiEnabled(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ enabled });
}

export async function setKasumiStealth(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ enable_stealth: enabled });
}

export async function setKasumiHidexattr(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ enable_hidexattr: enabled });
}

export async function setKasumiSelinuxFix(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ enable_selinux_fix: enabled });
}

export async function setKasumiDebug(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ enable_kernel_debug: enabled });
}

export async function getOriginalKernelUname(): Promise<KernelUnameValues> {
  const payload = await runDaemonCommand(
    { type: "api-kernel-uname" },
    PATHS.BINARY,
  );
  if (
    isRecord(payload) &&
    isString(payload.release) &&
    isString(payload.version)
  ) {
    return {
      release: payload.release.trim(),
      version: payload.version.trim(),
    };
  }
  throw new AppError("Failed to read original kernel uname values");
}

export async function setKasumiUnameMode(
  mode: "scoped" | "global",
): Promise<void> {
  await updateKasumiConfig({
    uname_mode: mode === "global" ? "global" : "scoped",
  });
}

export async function setKasumiUname(
  uname: Partial<KasumiUnameConfig>,
): Promise<void> {
  await updateKasumiConfig({ uname });
}

export async function applyKasumiUname(
  mode: "scoped" | "global",
  uname: Pick<KasumiUnameConfig, "release" | "version">,
): Promise<void> {
  const release = uname.release.trim();
  const version = uname.version.trim();
  if (!release || !version) {
    throw new AppError("uname release and version must both be non-empty");
  }
  await runDaemonCommand(
    {
      type: "kasumi-set-uname",
      mode: mode === "global" ? "global" : "scoped",
      release,
      version,
    },
    PATHS.BINARY,
  );
}

export async function clearKasumiUname(
  mode: "scoped" | "global" = "scoped",
): Promise<void> {
  await runDaemonCommand(
    {
      type: "kasumi-clear-uname",
      mode: mode === "global" ? "global" : "scoped",
    },
    PATHS.BINARY,
  );
  await updateKasumiConfig(
    {
      uname: {
        sysname: "",
        nodename: "",
        release: "",
        version: "",
        machine: "",
        domainname: "",
      },
    },
    { applyRuntime: false },
  );
}

export async function restoreKasumiUnameGlobal(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-restore-uname-global" }, PATHS.BINARY);
}

export async function setKasumiCmdline(value: string): Promise<void> {
  await updateKasumiConfig({ cmdline_value: value });
}

export async function clearKasumiCmdline(): Promise<void> {
  await updateKasumiConfig({ cmdline_value: "" });
}

export async function addKasumiMapsRule(rule: {
  target_ino: number;
  target_dev: number;
  spoofed_ino: number;
  spoofed_dev: number;
  spoofed_pathname: string;
}): Promise<void> {
  await runDaemonCommand({ type: "api-kasumi-maps-add", rule }, PATHS.BINARY);
}

export async function clearKasumiMapsRules(): Promise<void> {
  await runDaemonCommand({ type: "api-kasumi-maps-clear" }, PATHS.BINARY);
}

export async function getUserHideRules(): Promise<string[]> {
  const payload = await runDaemonCommand({ type: "hide-list" }, PATHS.BINARY);
  if (
    Array.isArray(payload) &&
    payload.every((item) => typeof item === "string")
  ) {
    return payload;
  }
  throw new AppError("hide list returned invalid payload");
}

export async function addUserHideRule(path: string): Promise<void> {
  await runDaemonCommand({ type: "hide-add", path }, PATHS.BINARY);
}

export async function removeUserHideRule(path: string): Promise<void> {
  await runDaemonCommand({ type: "hide-remove", path }, PATHS.BINARY);
}

export async function applyUserHideRules(): Promise<void> {
  await runDaemonCommand({ type: "hide-apply" }, PATHS.BINARY);
}

export async function loadKasumiLkm(): Promise<void> {
  await runDaemonCommand({ type: "lkm-load" }, PATHS.BINARY);
}

export async function unloadKasumiLkm(): Promise<void> {
  await runDaemonCommand({ type: "lkm-unload" }, PATHS.BINARY);
}

export async function setKasumiLkmAutoload(enabled: boolean): Promise<void> {
  await updateKasumiConfig({ lkm_autoload: enabled }, { applyRuntime: false });
}

export async function setKasumiLkmKmi(value: string): Promise<void> {
  await updateKasumiConfig(
    { lkm_kmi_override: value },
    { applyRuntime: false },
  );
}

export async function clearKasumiLkmKmi(): Promise<void> {
  await updateKasumiConfig({ lkm_kmi_override: "" }, { applyRuntime: false });
}

export async function fixKasumiMounts(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-fix-mounts" }, PATHS.BINARY);
}

export async function clearKasumiRules(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-clear" }, PATHS.BINARY);
}

export async function releaseKasumiConnection(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-release-connection" }, PATHS.BINARY);
}

export async function invalidateKasumiCache(): Promise<void> {
  await runDaemonCommand({ type: "kasumi-invalidate-cache" }, PATHS.BINARY);
}

export async function applyKasumiConfigRuntime(): Promise<void> {
  await applyKasumiRuntimeConfig();
}
