/*
 * Copyright (C) 2026 YuzakiKokuban <heibanbaize@gmail.com>
 *
 * Licensed under the Apache License, Version 2.0 (the "License");
 * you may not use this file except in compliance with the License.
 * You may obtain a copy of the License at
 *
 *     http://www.apache.org/licenses/LICENSE-2.0
 *
 * Unless required by applicable law or agreed to in writing, software
 * distributed under the License is distributed on an "AS IS" BASIS,
 * WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
 * See the License for the specific language governing permissions and
 * limitations under the License.
 */

import { defineConfig, loadEnv } from "vite";
import solid from "vite-plugin-solid";

function envFlagEnabled(value: string | undefined): boolean {
  return ["true", "1", "on"].includes(value?.trim().toLowerCase() ?? "");
}

export default defineConfig(({ command, mode }) => {
  const env = loadEnv(mode, ".", "");
  const useDevMockLoader =
    command === "serve" || mode === "test" || envFlagEnabled(env.VITE_USE_MOCK);
  const mockLoaderPath = useDevMockLoader
    ? "./src/lib/api.mock-loader.dev.ts"
    : "./src/lib/api.mock-loader.ts";

  return {
    base: "./",
    build: {
      outDir: "../module/webroot",
      target: "esnext",
    },
    plugins: [solid()],
    resolve: {
      alias: [
        {
          find: "./api.mock-loader",
          replacement: new URL(mockLoaderPath, import.meta.url).pathname,
        },
      ],
    },
  };
});
