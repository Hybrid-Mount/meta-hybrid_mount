// SPDX-License-Identifier: Apache-2.0
import { beforeEach, describe, expect, it, vi } from "vitest";

vi.mock("../api", () => ({ API: { getRuntimeStatus: vi.fn(), runtimeAction: vi.fn() } }));
vi.mock("./moduleStore", () => ({
  moduleStore: {
    loadModules: vi.fn().mockResolvedValue(undefined),
    get loading() {
      return false;
    },
  },
}));
vi.mock("./sysStore", () => ({
  sysStore: { loadStatus: vi.fn().mockResolvedValue(undefined) },
}));
import { API } from "../api";
import { moduleStore } from "./moduleStore";
import { sysStore } from "./sysStore";
import { runtimeStore } from "./runtimeStore";

const status = {
  supported: true,
  reason: null,
  generation: 3,
  modules: [
    { id: "ready", active: false, eligible: true, reason: null },
    { id: "mixed", active: true, eligible: false, reason: "Mixed backend ownership" },
  ],
};

beforeEach(async () => {
  vi.clearAllMocks();
  vi.mocked(API.getRuntimeStatus).mockResolvedValue(status);
  vi.mocked(API.runtimeAction).mockResolvedValue({ ok: true, generation: 3 });
  await runtimeStore.loadRuntimeStatus();
});

describe("runtime controls", () => {
  it("rejects ineligible or unknown modules before invoking the bridge", async () => {
    expect(await runtimeStore.runAction("mixed", "unload")).toBe(false);
    expect(await runtimeStore.runAction("unknown", "load")).toBe(false);
    expect(API.runtimeAction).not.toHaveBeenCalled();
  });
  it("locks all runtime actions until the operation and refresh finish", async () => {
    let finish!: () => void;
    vi.mocked(API.runtimeAction).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = () => resolve({ ok: true, generation: 3 });
        }),
    );
    const operation = runtimeStore.runAction("ready", "load");
    expect(runtimeStore.busy).toBe(true);
    expect(await runtimeStore.runAction("ready", "load")).toBe(false);
    finish();
    expect(await operation).toBe(true);
    expect(API.runtimeAction).toHaveBeenCalledTimes(1);
    expect(moduleStore.loadModules).toHaveBeenCalledOnce();
    expect(sysStore.loadStatus).toHaveBeenCalledOnce();
    expect(API.getRuntimeStatus).toHaveBeenCalledTimes(2);
    expect(runtimeStore.busy).toBe(false);
  });
  it("retains rejection details and re-reads actual runtime state after a failure", async () => {
    vi.mocked(API.runtimeAction).mockRejectedValueOnce(
      new Error("Provider rejected replacement"),
    );
    expect(await runtimeStore.runAction("ready", "load")).toBe(false);
    expect(runtimeStore.actionError).toEqual({
      moduleId: "ready",
      message: "Provider rejected replacement",
    });
    expect(API.getRuntimeStatus).toHaveBeenCalledTimes(2);
  });
  it("discards stale eligibility after a failed refresh", async () => {
    vi.mocked(API.getRuntimeStatus).mockRejectedValueOnce(
      new Error("ledger unavailable"),
    );
    await runtimeStore.loadRuntimeStatus();
    expect(runtimeStore.status).toBeNull();
    expect(runtimeStore.loadError).toBe("ledger unavailable");
    expect(await runtimeStore.runAction("ready", "load")).toBe(false);
    expect(API.runtimeAction).not.toHaveBeenCalled();
  });
  it("unlocks controls and reports a snapshot refresh rejection without rejecting the action promise", async () => {
    vi.mocked(moduleStore.loadModules).mockRejectedValueOnce(
      new Error("scan unavailable"),
    );
    await expect(runtimeStore.runAction("ready", "load")).resolves.toBe(false);
    expect(runtimeStore.busy).toBe(false);
    expect(runtimeStore.status).toBeNull();
    expect(runtimeStore.loadError).toContain("scan unavailable");
  });
  it("rejects a runtime snapshot older than the acknowledged mutation", async () => {
    vi.mocked(API.getRuntimeStatus).mockResolvedValueOnce({ ...status, generation: 2 });
    expect(await runtimeStore.runAction("ready", "load")).toBe(false);
    expect(runtimeStore.status).toBeNull();
    expect(runtimeStore.loadError).toContain("stale");
    expect(runtimeStore.busy).toBe(false);
  });
  it("updates runtime activity from the fresh post-operation snapshot", async () => {
    vi.mocked(API.getRuntimeStatus).mockResolvedValueOnce({
      ...status,
      modules: [{ id: "ready", active: true, eligible: true, reason: null }],
    });
    expect(await runtimeStore.runAction("ready", "load")).toBe(true);
    expect(runtimeStore.status?.modules[0]?.active).toBe(true);
    expect(await runtimeStore.runAction("ready", "load")).toBe(false);
    expect(API.runtimeAction).toHaveBeenCalledTimes(1);
  });

  it("waits for an older module scan and then obtains a fresh post-operation scan", async () => {
    const loading = vi.spyOn(moduleStore, "loading", "get").mockReturnValueOnce(true);
    let finish!: () => void;
    vi.mocked(moduleStore.loadModules).mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    const operation = runtimeStore.runAction("ready", "load");
    await Promise.resolve();
    expect(runtimeStore.busy).toBe(true);
    expect(moduleStore.loadModules).toHaveBeenCalledTimes(1);
    finish();
    expect(await operation).toBe(true);
    expect(moduleStore.loadModules).toHaveBeenCalledTimes(2);
    loading.mockRestore();
  });
});
