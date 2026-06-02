import { PATHS } from "../../constants";
import { ENABLE_KASUMI } from "../../constants_gen";
import type { AppConfig } from "../../types";
import { runDaemonCommand } from "../core/bridge";
import { isRecord } from "../core/guards";
import { normalizeConfig } from "../codec/configCodec";

export function extractConfig(payload: unknown): AppConfig {
  // api-config-patch and api-config-set responses wrap the updated config in a `config` field;
  // api-config-get returns the config object directly.
  if (isRecord(payload) && isRecord(payload.config)) {
    return normalizeConfig(payload.config);
  }
  return normalizeConfig(payload);
}

export async function loadConfigFromFile(): Promise<AppConfig> {
  const payload = await runDaemonCommand(
    { type: "api-config-get" },
    PATHS.BINARY,
  );
  return normalizeConfig(payload);
}

export async function saveConfigToFile(config: AppConfig): Promise<void> {
  const normalized = normalizeConfig(config);
  const patch = {
    moduledir: normalized.moduledir,
    mountsource: normalized.mountsource,
    overlay_mode: normalized.overlay_mode,
    disable_umount: normalized.disable_umount,
    default_mode: normalized.default_mode,
    daemon_startup_mode: normalized.daemon_startup_mode,
    rules: normalized.rules,
    ...(ENABLE_KASUMI ? { kasumi: normalized.kasumi } : {}),
  };
  await patchConfigFile(patch);
}

export async function patchConfigFile(
  patch: Record<string, unknown>,
  options: { applyRuntime?: boolean } = {},
): Promise<AppConfig> {
  const payload = await runDaemonCommand(
    {
      type: "api-config-patch",
      patch,
      apply_runtime: options.applyRuntime !== false,
    },
    PATHS.BINARY,
  );
  return extractConfig(payload);
}

export async function resetConfigFile(): Promise<AppConfig> {
  const payload = await runDaemonCommand(
    { type: "api-config-reset" },
    PATHS.BINARY,
  );
  return extractConfig(payload);
}
