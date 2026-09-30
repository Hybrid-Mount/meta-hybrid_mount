// SPDX-License-Identifier: Apache-2.0
import { describe, expect, it, vi } from "vitest";
import { createRuntimeApi } from "./runtimeApi";

describe("runtime bridge", () => {
  it("reads eligibility from runtime status without inventing permission", async () => {
    const exec = vi.fn().mockResolvedValue({
      errno: 0,
      stdout: JSON.stringify({
        supported: true,
        generation: 4,
        modules: [{ id: "demo", active: true, eligible: "true" }],
      }),
      stderr: "",
    });
    const status = await createRuntimeApi(exec).getRuntimeStatus();
    expect(exec).toHaveBeenCalledWith(expect.stringMatching(/ runtime status$/));
    expect(status.modules).toEqual([
      { id: "demo", active: true, eligible: false, reason: null },
    ]);
  });
  it.each(["load", "unload", "reload"] as const)(
    "quotes module identifiers when sending %s",
    async (action) => {
      const exec = vi.fn().mockResolvedValue({
        errno: 0,
        stdout: '{"ok":true,"generation":5}',
        stderr: "",
      });
      await expect(
        createRuntimeApi(exec).runtimeAction('demo"$`\\ name', action),
      ).resolves.toEqual({ ok: true, generation: 5 });
      expect(exec).toHaveBeenCalledWith(
        expect.stringContaining(" runtime " + action + ' "demo\\"\\$\\`\\\\ name"'),
      );
    },
  );
  it("preserves backend rejection diagnostics", async () => {
    const exec = vi.fn().mockResolvedValue({
      errno: 1,
      stdout: "",
      stderr: "Mixed backend ownership prevents unload",
    });
    await expect(createRuntimeApi(exec).runtimeAction("demo", "unload")).rejects.toThrow(
      "Mixed backend ownership prevents unload",
    );
  });
  it("rejects missing success acknowledgments", async () => {
    const exec = vi
      .fn()
      .mockResolvedValue({ errno: 0, stdout: '{"ok":false,"generation":3}', stderr: "" });
    await expect(createRuntimeApi(exec).runtimeAction("demo", "load")).rejects.toThrow();
  });
  it("does not turn a failed status read into an empty supported runtime", async () => {
    const exec = vi
      .fn()
      .mockResolvedValue({ errno: 1, stdout: "", stderr: "ledger unavailable" });
    await expect(createRuntimeApi(exec).getRuntimeStatus()).rejects.toThrow(
      "ledger unavailable",
    );
  });
});
