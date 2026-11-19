// SPDX-License-Identifier: Apache-2.0

const MAX_WEAR_VIEWPORT_WIDTH = 280;
const WATCH_PROBE_TIMEOUT_MS = 1000;

export async function useWearLayout(
  search: string,
  viewportWidth: number,
  isWatch: () => Promise<boolean>,
): Promise<boolean> {
  const override = new URLSearchParams(search).get("layout");
  if (override === "wear") return true;
  if (override === "phone") return false;
  if (viewportWidth <= 0 || viewportWidth > MAX_WEAR_VIEWPORT_WIDTH) return false;

  let timeout: ReturnType<typeof setTimeout> | undefined;
  const fallback = new Promise<false>((resolve) => {
    timeout = setTimeout(() => resolve(false), WATCH_PROBE_TIMEOUT_MS);
  });
  try {
    return await Promise.race([isWatch(), fallback]);
  } catch {
    return false;
  } finally {
    clearTimeout(timeout);
  }
}
