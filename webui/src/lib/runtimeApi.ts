// SPDX-License-Identifier: Apache-2.0
import { PATHS } from "./constants";
import { shellEscapeDoubleQuoted } from "./shell";
import type { AppAPI, RuntimeStatus } from "./types";

type Exec = (
  command: string,
) => Promise<{ errno: number; stdout: string; stderr: string }>;

export function createRuntimeApi(
  exec: Exec,
): Pick<AppAPI, "getRuntimeStatus" | "runtimeAction"> {
  return {
    async getRuntimeStatus(): Promise<RuntimeStatus> {
      const { errno, stdout, stderr } = await exec(`${PATHS.BINARY} runtime status`);
      if (errno !== 0 || !stdout.trim())
        throw new Error(stderr || "runtime status failed");
      const payload = JSON.parse(stdout);
      return {
        supported: payload.supported === true,
        reason: typeof payload.reason === "string" ? payload.reason : null,
        generation: Number(payload.generation ?? 0),
        modules: Array.isArray(payload.modules)
          ? payload.modules.map((item: Record<string, unknown>) => ({
              id: String(item.id ?? ""),
              active: item.active === true,
              eligible: item.eligible === true,
              reason: typeof item.reason === "string" ? item.reason : null,
            }))
          : [],
      };
    },
    async runtimeAction(moduleId, action) {
      if (!["load", "unload", "reload"].includes(action))
        throw new Error("Unknown runtime action");
      const { errno, stdout, stderr } = await exec(
        `${PATHS.BINARY} runtime ${action} "${shellEscapeDoubleQuoted(moduleId)}"`,
      );
      if (errno !== 0 || !stdout.trim())
        throw new Error(stderr || `runtime ${action} failed`);
      const result = JSON.parse(stdout);
      if (
        result.ok !== true ||
        !Number.isSafeInteger(result.generation) ||
        result.generation < 0
      ) {
        throw new Error(`runtime ${action} returned an invalid acknowledgment`);
      }
      return { ok: true, generation: result.generation };
    },
  };
}
