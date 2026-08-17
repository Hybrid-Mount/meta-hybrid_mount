// SPDX-License-Identifier: Apache-2.0

// 一次性迁移脚本:把旧历史 locale(React key 结构)按语义映射到
// vue-i18n 的新 key 集。被删除功能的 key 不迁移,缺失 key 由 en fallback。
// 用法: node scripts/migrate-locales.mjs <old-locales-dir> <out-dir>

import { readFileSync, writeFileSync, mkdirSync } from "node:fs";
import { join, basename } from "node:path";

const sourceDir = process.argv[2];
const outDir = process.argv[3];

if (!sourceDir || !outDir) {
  console.error("usage: node migrate-locales.mjs <old-locales-dir> <out-dir>");
  process.exit(1);
}

const mapping = {
  "lang.display": ["lang", "display"],
  "common.appName": ["common", "appName"],
  "common.brand": ["common", "appName"],
  "common.saving": ["common", "saving"],
  "common.cancel": ["common", "cancel"],
  "common.close": ["common", "cancel"],
  "common.reboot": ["common", "reboot"],
  "common.rebootTitle": ["common", "rebootTitle"],
  "common.rebootConfirm": ["common", "rebootConfirm"],
  "common.language": ["common", "language"],
  "common.enabled": ["config", "kasumiStateEnabled"],
  "common.disabled": ["config", "kasumiStateDisabled"],
  "tabs.status": ["tabs", "status"],
  "tabs.config": ["tabs", "config"],
  "tabs.modules": ["tabs", "modules"],
  "tabs.info": ["tabs", "info"],
  "status.backendTitle": ["status", "storageTitle"],
  "status.sysInfoTitle": ["status", "sysInfoTitle"],
  "status.moduleActive": ["status", "moduleActive"],
  "status.kernelLabel": ["status", "kernel"],
  "status.selinuxLabel": ["status", "selinux"],
  "status.loadError": ["status", "loadError"],
  "config.moduledir": ["config", "moduleDir"],
  "config.moduledirDesc": ["config", "moduleDirDesc"],
  "config.mountSource": ["config", "mountSource"],
  "config.mountSourceDesc": ["config", "mountSourceDesc"],
  "config.overlayMode": ["config", "overlayMode"],
  "config.overlayModeDesc": ["config", "overlayModeDesc"],
  "config.overlayTmpfs": ["config", "mode_tmpfs"],
  "config.overlayExt4": ["config", "mode_ext4"],
  "config.disableUmount": ["config", "disableUmount"],
  "config.moduleDefault": ["modules", "defaultMode"],
  "config.modeOverlay": ["modules", "modes", "overlay"],
  "config.modeMagic": ["modules", "modes", "magic"],
  "config.modeIgnore": ["modules", "modes", "short", "ignore"],
  "config.save": ["config", "save"],
  "config.reset": ["config", "resetConfig"],
  "config.saveSuccess": ["common", "saved"],
  "config.loadError": ["config", "loadError"],
  "config.saveFailed": ["config", "saveFailed"],
  "config.resetSuccess": ["config", "resetSuccess"],
  "modules.reload": ["modules", "reload"],
  "modules.save": ["modules", "save"],
  "modules.empty": ["modules", "emptyState"],
  "modules.scanError": ["modules", "scanError"],
  "modules.saveSuccess": ["modules", "saveSuccess"],
  "modules.saveFailed": ["modules", "saveFailed"],
  "modules.searchPlaceholder": ["modules", "searchPlaceholder"],
  "modules.filterLabel": ["modules", "filterLabel"],
  "modules.filterAll": ["modules", "filterAll"],
  "modules.modeOverlay": ["modules", "modes", "overlay"],
  "modules.modeMagic": ["modules", "modes", "magic"],
  "modules.modeIgnore": ["modules", "modes", "short", "ignore"],
  "modules.clearErrors": ["modules", "clearMountErrors"],
  "modules.clearErrorsSuccess": ["modules", "mountErrorsCleared"],
  "modules.mountError": ["modules", "mountError"],
  "modules.suggestIgnore": ["modules", "suggestIgnoreHint"],
  "info.projectLink": ["info", "projectLink"],
  "info.contributors": ["info", "contributors"],
  "info.loadFail": ["info", "loadFail"],
  "info.noBio": ["info", "noBio"],
};

const codes = [
  "es-ES",
  "id-ID",
  "it-IT",
  "ja-JP",
  "ru-RU",
  "uk-UA",
  "vi-VN",
  "zh-TW",
  "tr-TR",
];

function getAt(source, path) {
  let cursor = source;
  for (const part of path) {
    if (!cursor || typeof cursor !== "object") return undefined;
    cursor = cursor[part];
  }
  return typeof cursor === "string" && cursor.trim().length > 0 ? cursor : undefined;
}

function setAt(target, path, value) {
  let cursor = target;
  for (let index = 0; index < path.length - 1; index += 1) {
    const part = path[index];
    cursor[part] ??= {};
    cursor = cursor[part];
  }
  cursor[path[path.length - 1]] = value;
}

mkdirSync(outDir, { recursive: true });

for (const code of codes) {
  const raw = readFileSync(join(sourceDir, `${code}.json`), "utf8").replace(
    /^\uFEFF/,
    "",
  );
  const source = JSON.parse(raw);
  const target = {};

  for (const [targetKey, sourcePath] of Object.entries(mapping)) {
    const value = getAt(source, sourcePath);
    if (value) setAt(target, targetKey.split("."), value);
  }

  const display = getAt(source, ["lang", "display"]);
  if (display) setAt(target, ["lang", "display"], display);

  const targetPath = join(outDir, `${code}.json`);
  writeFileSync(targetPath, `${JSON.stringify(target, null, 2)}\n`);
  console.log(
    `migrated ${basename(targetPath)} (${Object.keys(mapping).length + 1} keys)`,
  );
}
