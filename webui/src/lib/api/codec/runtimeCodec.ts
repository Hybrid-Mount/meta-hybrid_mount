import type { AppConfig, KasumiStatus, StorageStatus } from "../../types";
import type {
  RuntimeModeStatsPayload,
  RuntimeStatePayload,
} from "../repos/runtimeRepo";
import { ENABLE_KASUMI } from "../../constants_gen";
import {
  isBoolean,
  isNumber,
  isRecord,
  isString,
  isStringArray,
  toNonNegativeInt,
} from "../core/guards";
import { normalizeKasumiConfig } from "./configCodec";

export function buildModeStats(
  state: RuntimeStatePayload,
): NonNullable<StorageStatus["modeStats"]> {
  const modeStats = isRecord(state.mode_stats)
    ? (state.mode_stats as RuntimeModeStatsPayload)
    : {};
  return {
    overlay: toNonNegativeInt(modeStats.overlayfs),
    magic: toNonNegativeInt(modeStats.magicmount),
    kasumi: ENABLE_KASUMI ? toNonNegativeInt(modeStats.kasumi) : 0,
    blacklisted: toNonNegativeInt(modeStats.blacklisted),
  };
}

export function buildMountedCount(
  state: RuntimeStatePayload,
  modeStats: NonNullable<StorageStatus["modeStats"]>,
): number {
  const overlay = isStringArray(state.overlay_modules)
    ? state.overlay_modules.length
    : 0;
  const magic = isStringArray(state.magic_modules)
    ? state.magic_modules.length
    : 0;
  const kasumi =
    ENABLE_KASUMI && isStringArray(state.kasumi_modules)
      ? state.kasumi_modules.length
      : 0;
  const total = overlay + magic + kasumi;
  return total > 0
    ? total
    : modeStats.overlay + modeStats.magic + modeStats.kasumi;
}

export function buildKasumiStatusFromPayload(
  payload: unknown,
  fallbackConfig: AppConfig["kasumi"],
  fallbackState: RuntimeStatePayload,
): KasumiStatus | null {
  if (!isRecord(payload)) return null;

  const lkmPayload = isRecord(payload.lkm) ? payload.lkm : {};
  const configPayload = isRecord(payload.config)
    ? payload.config
    : fallbackConfig;
  const runtimePayload = isRecord(payload.runtime) ? payload.runtime : {};

  return {
    status: isString(payload.status)
      ? payload.status
      : fallbackConfig.enabled
        ? "unavailable"
        : "disabled",
    available: isBoolean(payload.available) ? payload.available : false,
    kernel_supported: isBoolean(payload.kernel_supported)
      ? payload.kernel_supported
      : false,
    protocol_version:
      payload.protocol_version === null || isNumber(payload.protocol_version)
        ? ((payload.protocol_version as number | null | undefined) ?? null)
        : null,
    feature_bits:
      payload.feature_bits === null || isNumber(payload.feature_bits)
        ? ((payload.feature_bits as number | null | undefined) ?? null)
        : null,
    feature_names: isStringArray(payload.feature_names)
      ? payload.feature_names
      : [],
    hooks: isStringArray(payload.hooks) ? payload.hooks : [],
    rule_count: toNonNegativeInt(payload.rule_count),
    user_hide_rule_count: toNonNegativeInt(payload.user_hide_rule_count),
    mirror_path: isString(payload.mirror_path)
      ? payload.mirror_path
      : fallbackConfig.mirror_path,
    lkm: {
      loaded: isBoolean(lkmPayload.loaded) ? lkmPayload.loaded : false,
      module_name: isString(lkmPayload.module_name)
        ? lkmPayload.module_name
        : undefined,
      autoload: isBoolean(lkmPayload.autoload)
        ? lkmPayload.autoload
        : fallbackConfig.lkm_autoload,
      kmi_override: isString(lkmPayload.kmi_override)
        ? lkmPayload.kmi_override
        : fallbackConfig.lkm_kmi_override,
      current_kmi: isString(lkmPayload.current_kmi)
        ? lkmPayload.current_kmi
        : "",
      search_dir: isString(lkmPayload.search_dir)
        ? lkmPayload.search_dir
        : fallbackConfig.lkm_dir,
      module_file: isString(lkmPayload.module_file)
        ? lkmPayload.module_file
        : undefined,
      last_error:
        lkmPayload.last_error === null || isString(lkmPayload.last_error)
          ? ((lkmPayload.last_error as string | null | undefined) ?? null)
          : null,
    },
    config: normalizeKasumiConfig(configPayload),
    runtime: {
      snapshot: isRecord(runtimePayload.snapshot)
        ? (runtimePayload.snapshot as Record<string, unknown>)
        : isRecord(fallbackState.kasumi)
          ? (fallbackState.kasumi as Record<string, unknown>)
          : {},
      kasumi_modules: isStringArray(runtimePayload.kasumi_modules)
        ? runtimePayload.kasumi_modules
        : isStringArray(fallbackState.kasumi_modules)
          ? fallbackState.kasumi_modules
          : [],
    },
  };
}
