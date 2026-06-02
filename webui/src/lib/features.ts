import { createRoot, createSignal } from "solid-js";
import { ENABLE_KASUMI } from "./constants_gen";

const createFeatures = () => {
  const [kasumiEnabled, setKasumiEnabled] = createSignal(false);
  const [kasumiAvailable, setKasumiAvailable] = createSignal(false);
  const [kasumiKernelSupported, setKasumiKernelSupported] = createSignal(false);

  return {
    get kasumiBuildEnabled() {
      return ENABLE_KASUMI;
    },
    get kasumiEnabled() {
      return ENABLE_KASUMI && kasumiEnabled();
    },
    get kasumiAvailable() {
      return ENABLE_KASUMI && kasumiAvailable();
    },
    get kasumiKernelSupported() {
      return ENABLE_KASUMI && kasumiKernelSupported();
    },
    setKasumiStatus(
      enabled: boolean,
      available: boolean,
      kernelSupported: boolean,
    ) {
      if (!ENABLE_KASUMI) return;
      setKasumiEnabled(enabled);
      setKasumiAvailable(available);
      setKasumiKernelSupported(kernelSupported);
    },
  };
};

export const features = createRoot(createFeatures);
