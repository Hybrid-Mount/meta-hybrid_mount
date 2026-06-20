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

export interface MessageSchema {
  lang: {
    display: string;
  };
  common: {
    appName: string;
    saving: string;
    language: string;
    rebootTitle: string;
    rebootConfirm: string;
    reboot: string;
    cancel: string;
    saved: string;
  };
  tabs: {
    status: string;
    config: string;
    kasumi: string;
    modules: string;
    info: string;
  };
  status: {
    storageTitle: string;
    moduleActive: string;
    modeStats: string;
    sysInfoTitle: string;
    kernel: string;
    selinux: string;
    mountBase: string;
    activePartitions: string;
    loadError: string;
  };
  config: {
    disableUmount: string;
    moduleDir: string;
    moduleDirDesc: string;
    mountSource: string;
    mountSourceDesc: string;
    reload: string;
    save: string;
    resetConfig: string;
    resetConfigTitle: string;
    resetConfigConfirm: string;
    invalidPath: string;
    invalidModuleDir: string;
    loadError: string;
    saveFailed: string;
    resetSuccess: string;
    kasumiMasterTitle: string;
    kasumiMasterDesc: string;
    kasumiMasterSwitch: string;
    kasumiStateEnabled: string;
    kasumiStateDisabled: string;
    kasumiWarningTitle: string;
    kasumiWarningBody: string;
    kasumiEnableConfirm: string;
    kasumiConfigSaved: string;
    kasumiEnabledSuccess: string;
    kasumiDisabledSuccess: string;
    webui: string;
    experimentalFeatures: string;
    overlayMode: string;
    overlayModeDesc: string;
    mode_tmpfs: string;
    mode_tmpfsDesc: string;
    mode_ext4: string;
    mode_ext4Desc: string;
    daemonStartupMode: string;
  };
  modules: {
    reload: string;
    save: string;
    scanError: string;
    saveSuccess: string;
    saveFailed: string;
    searchPlaceholder: string;
    filterLabel: string;
    filterAll: string;
    loadMore: string;
    defaultMode: string;
    modes: {
      overlay: string;
      magic: string;
      kasumi: string;
      unmounted: string;
      short: {
        overlay: string;
        magic: string;
        kasumi: string;
        ignore: string;
      };
    };
    emptyState: string;
    unmountedHiddenHint: string;
    defaultTag: string;
    compatTag: string;
    disableTag: string;
    nativeTag: string;
    unavailableTag: string;
    kasumiUnavailableHint: string;
    mountError: string;
    suggestIgnoreHint: string;
    clearMountErrors: string;
    mountErrorsCleared: string;
    mountErrorsClearFailed: string;
  };
  kasumi: {
    title: string;
    statusLoading: string;
    statusWorking: string;
    statusUnavailable: string;
    statusDisabled: string;
    statusConfigOffRuntimeOn: string;
    rulesBadge: string;
    runtimeTitle: string;
    lkmTitle: string;
    identityTitle: string;
    featuresTitle: string;
    userHideTitle: string;
    mapsTitle: string;
    mapsAddRule: string;
    mapsClear: string;
    mapsRuleAdded: string;
    mapsCleared: string;
    mapsEmpty: string;
    mapsPathRequired: string;
    mapsTargetIno: string;
    mapsTargetDev: string;
    mapsSpoofedIno: string;
    mapsSpoofedDev: string;
    mapsSpoofedPath: string;
    mapsRuleSummary: string;
    rulesTitle: string;
    refresh: string;
    fixMounts: string;
    clearRules: string;
    loadLkm: string;
    unloadLkm: string;
    unloadLkmWarningTitle: string;
    unloadLkmWarningBody: string;
    unloadLkmWarningCancel: string;
    unloadLkmWarningConfirm: string;
    saveKmi: string;
    autoloadOn: string;
    autoloadOff: string;
    enableAutoload: string;
    disableAutoload: string;
    autoloadUpdated: string;
    fillOriginalKernel: string;
    originalKernelLoaded: string;
    originalKernelLoadFailed: string;
    unameSpoofTitle: string;
    unameEmptyHint: string;
    saveUname: string;
    applyUname: string;
    clearUname: string;
    restoreUnameGlobal: string;
    saveCmdline: string;
    applyCmdline: string;
    clearCmdline: string;
    addHideRule: string;
    removeHideRule: string;
    applyHideRules: string;
    hideRuleAdded: string;
    hideRuleRemoved: string;
    hideRulesApplied: string;
    noUserHideRules: string;
    userHidePathLabel: string;
    userHidePathRequired: string;
    stealthUpdated: string;
    hidexattrUpdated: string;
    selinuxFixUpdated: string;
    ignoreProtocolUpdated: string;
    kernelDebugUpdated: string;
    kmiOverride: string;
    autoKmi: string;
    currentKmi: string;
    lastError: string;
    stealthTitle: string;
    hidexattrTitle: string;
    selinuxFixTitle: string;
    ignoreProtocolTitle: string;
    ignoreProtocolHint: string;
    lkmUnavailableHint: string;
    configOnlyHint: string;
    kernelDebugTitle: string;
    featureBits: string;
    hideUidCount: string;
    userHideCount: string;
    mapsRuleCount: string;
    kstatRuleCount: string;
    unameSysname: string;
    unameNodename: string;
    unameMachine: string;
    unameDomainname: string;
    unameMode: string;
    unameModeScoped: string;
    unameModeGlobal: string;
    unameModeScopedDesc: string;
    unameModeGlobalDesc: string;
    unameRelease: string;
    unameReleaseDesc: string;
    unameVersion: string;
    unameVersionDesc: string;
    cmdlineValue: string;
    rulesVisibleSummary: string;
    dtypeLabel: string;
    loadError: string;
  };
  logs: {
    refresh: string;
  };
  info: {
    projectLink: string;
    donate: string;
    contributors: string;
    loadFail: string;
    noBio: string;
    telegram: string;
    supportUs: string;
    authorYuzaki: string;
  };
}
