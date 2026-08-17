// SPDX-License-Identifier: Apache-2.0

/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly MODULE_ID: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}

declare module "miuix-vue";
declare module "miuix-vue/icons";
