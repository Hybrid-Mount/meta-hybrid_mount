// SPDX-License-Identifier: Apache-2.0

export function parseLateLoad(output: string): boolean {
  const fields = output.split(/\r?\n/).filter((line) => /^\s*late_load\b/.test(line));
  const match =
    fields.length === 1 ? fields[0]?.match(/^\s*late_load\s*:\s*(true|false)\s*$/) : null;
  if (!match) {
    throw new Error("Cannot verify KernelSU late-load mode; reboot cancelled");
  }
  return match[1] === "true";
}

export function parseRootManager(output: string): "kernelsu" | "apatch" | null {
  const match = output.match(/^KSU=(true|false|)\r?\nAPATCH=(true|false|)\r?\n?$/);
  if (!match || (match[1] === "true" && match[2] === "true")) {
    throw new Error("Cannot verify root environment; reboot cancelled");
  }
  if (match[1] === "true") return "kernelsu";
  if (match[2] === "true") return "apatch";
  // Manager shells do not always export the installer environment markers.
  return null;
}
