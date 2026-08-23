import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

let moduleId = process.env.MODULE_ID;

if (!moduleId) {
  moduleId = "hybrid_mount";
}

export default defineConfig({
  base: "./",
  build: {
    outDir: "../module/webroot",
    target: "esnext",
    chunkSizeWarningLimit: 1000,
  },
  define: {
    "import.meta.env.MODULE_ID": JSON.stringify(moduleId),
  },
  plugins: [
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith("md-"),
        },
      },
    }),
  ],
});
