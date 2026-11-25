// SPDX-License-Identifier: Apache-2.0

export function loadOptionalManagerColors(): void {
  if (document.head.querySelector('link[data-manager-colors="true"]')) return;

  const link = document.createElement("link");
  link.rel = "stylesheet";
  link.href = "https://mui.kernelsu.org/internal/colors.css";
  link.dataset.managerColors = "true";
  link.addEventListener("error", () => link.remove(), { once: true });
  document.head.append(link);
}
