declare global {
  interface Window {
    litDisableBundleWarning: boolean;
  }
}

window.litDisableBundleWarning = true;

export {};