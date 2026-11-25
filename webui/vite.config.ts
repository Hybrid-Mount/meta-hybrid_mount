// SPDX-License-Identifier: Apache-2.0

import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";
import { fileURLToPath } from "node:url";

const flavor = process.env.WEBUI_FLAVOR ?? "normal";
if (flavor !== "normal" && flavor !== "wear") {
  throw new Error(`Unknown WEBUI_FLAVOR: ${flavor}`);
}

let moduleId = process.env.MODULE_ID;

if (!moduleId) {
  moduleId = "hybrid_mount";
}

export default defineConfig({
  base: "./",
  build: {
    outDir: `../output/webui-${flavor}`,
    target: "esnext",
    chunkSizeWarningLimit: 1000,
  },
  define: {
    "import.meta.env.MODULE_ID": JSON.stringify(moduleId),
  },
  resolve: {
    alias: {
      "@webui-entry": fileURLToPath(
        new URL(
          flavor === "wear" ? "./src/main-wear.ts" : "./src/main.ts",
          import.meta.url,
        ),
      ),
    },
  },
  plugins: [
    {
      name: "verify-flavor-layouts",
      generateBundle() {
        for (const id of this.getModuleIds()) {
          const path = id.replaceAll("\\", "/");
          const forbidden =
            flavor === "wear"
              ? path.includes("/ui/miuix/") || path.includes("/ui/md3/Md3Layout.vue")
              : path.includes("/ui/wear/");
          if (forbidden) this.error(`Unexpected ${flavor} layout dependency: ${id}`);
        }
      },
    },
    vue({
      template: {
        compilerOptions: {
          isCustomElement: (tag) => tag.startsWith("md-"),
        },
      },
    }),
  ],
});
