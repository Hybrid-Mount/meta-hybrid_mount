import { For, Show, createMemo, createSignal, onMount } from "solid-js";
import { createStore } from "solid-js/store";
import { API } from "../../lib/api";
import { ICONS } from "../../lib/constants";
import { uiStore } from "../../lib/stores/uiStore";
import { kasumiStore } from "../../lib/stores/kasumiStore";
import type { KasumiStatus } from "../../lib/types";
import { getErrorMessage } from "../../lib/api/core/error";
import BottomActions from "../../components/BottomActions";
import HeroCard from "./HeroCard";
import LkmSection from "./LkmSection";
import RuntimeSection from "./RuntimeSection";
import IdentitySection from "./IdentitySection";
import UserHideSection from "./UserHideSection";
import MapsSection from "./MapsSection";
import FeaturesSection from "./FeaturesSection";
import type { RefreshMode } from "./types";
import "../KasumiTab.css";
import "../StatusTab.css";

import "@material/web/button/filled-button.js";
import "@material/web/button/outlined-button.js";
import "@material/web/button/text-button.js";
import "@material/web/dialog/dialog.js";
import "@material/web/icon/icon.js";
import "@material/web/iconbutton/filled-tonal-icon-button.js";
import "@material/web/list/list.js";
import "@material/web/list/list-item.js";
import "@material/web/ripple/ripple.js";
import "@material/web/textfield/outlined-text-field.js";

const KNOWN_KMI_OPTIONS = [
  "android12-5.10",
  "android13-5.10",
  "android13-5.15",
  "android14-5.15",
  "android14-6.1",
  "android15-6.6",
  "android16-6.12",
] as const;

const USER_HIDE_RULES_CACHE_TTL_MS = 3000;

export default function KasumiTab() {
  const [userHideRules, setUserHideRules] = createSignal<string[]>([]);
  const [loading, setLoading] = createSignal(true);
  const [pending, setPending] = createSignal(false);
  const [showKmiDialog, setShowKmiDialog] = createSignal(false);
  const [showUnloadLkmWarning, setShowUnloadLkmWarning] = createSignal(false);
  const [expandedSection, setExpandedSection] = createSignal<string | null>(
    null,
  );
  const [forms, setForms] = createStore({
    kmi: "",
    unameMode: "scoped" as "scoped" | "global",
    release: "",
    version: "",
    cmdline: "",
    mapsTargetIno: "",
    mapsTargetDev: "0",
    mapsSpoofedIno: "",
    mapsSpoofedDev: "0",
    mapsPath: "",
    userHidePath: "",
  });

  let pendingUserHideRulesLoad: Promise<void> | null = null;
  let hasLoadedUserHideRules = false;
  let userHideRulesLoadedAt = 0;

  function hasFreshUserHideRules() {
    return (
      hasLoadedUserHideRules &&
      Date.now() - userHideRulesLoadedAt < USER_HIDE_RULES_CACHE_TTL_MS
    );
  }

  function syncForms(nextStatus: KasumiStatus) {
    const config = nextStatus.config;
    const uname = config.uname || {
      sysname: "",
      nodename: "",
      release: "",
      version: "",
      machine: "",
      domainname: "",
    };

    setForms({
      kmi: nextStatus.lkm?.kmi_override || config.lkm_kmi_override || "",
      unameMode: config.uname_mode === "global" ? "global" : "scoped",
      release: uname.release || "",
      version: uname.version || "",
      cmdline: config.cmdline_value || "",
      mapsTargetIno: "",
      mapsTargetDev: "0",
      mapsSpoofedIno: "",
      mapsSpoofedDev: "0",
      mapsPath: "",
      userHidePath: "",
    });
  }

  async function refreshStatusOnly(force = false) {
    if (force) {
      await kasumiStore.refreshStatus(true, true);
    } else {
      await kasumiStore.ensureStatusLoaded();
    }
    const nextStatus = kasumiStore.status;
    if (nextStatus) {
      syncForms(nextStatus);
    }
  }

  async function loadUserHideRules(force = false) {
    if (pendingUserHideRulesLoad) return pendingUserHideRulesLoad;
    if (!force && hasFreshUserHideRules()) return Promise.resolve();

    pendingUserHideRulesLoad = (async () => {
      const nextUserHideRules = await API.getUserHideRules();
      setUserHideRules(nextUserHideRules);
      hasLoadedUserHideRules = true;
      userHideRulesLoadedAt = Date.now();
    })();

    try {
      await pendingUserHideRulesLoad;
    } finally {
      pendingUserHideRulesLoad = null;
    }
  }

  async function load(mode: RefreshMode = "full") {
    setLoading(true);
    try {
      if (mode === "full") {
        await Promise.all([refreshStatusOnly(true), loadUserHideRules(true)]);
      } else {
        await refreshStatusOnly(true);
      }
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.kasumi?.loadError ?? "Failed to load Kasumi",
        ),
        "error",
      );
    } finally {
      setLoading(false);
    }
  }

  async function initialize() {
    setLoading(true);
    try {
      await refreshStatusOnly(false);
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.kasumi?.loadError ?? "Failed to load Kasumi",
        ),
        "error",
      );
    } finally {
      setLoading(false);
    }
  }

  async function runAction(
    action: () => Promise<void>,
    success: string,
    refreshMode: RefreshMode = "status-only",
  ) {
    setPending(true);
    try {
      await action();
      await load(refreshMode);
      uiStore.showToast(success, "success");
    } catch (e: unknown) {
      uiStore.showToast(getErrorMessage(e, "Action failed"), "error");
    } finally {
      setPending(false);
    }
  }

  async function fillOriginalKernelUname() {
    setPending(true);
    try {
      const original = await API.getOriginalKernelUname();
      setForms("release", original.release || "");
      setForms("version", original.version || "");
      uiStore.showToast(
        uiStore.L.kasumi?.originalKernelLoaded ??
          "Loaded original kernel values",
        "success",
      );
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.kasumi?.originalKernelLoadFailed ??
            "Failed to read original kernel values",
        ),
        "error",
      );
    } finally {
      setPending(false);
    }
  }

  async function saveAndApplyUname() {
    const release = forms.release.trim();
    const version = forms.version.trim();

    await API.setKasumiUnameMode(forms.unameMode);
    await API.setKasumiUname({ release, version });
    if (release && version) {
      await API.applyKasumiUname(forms.unameMode, {
        release,
        version,
      });
    }
  }

  async function clearUname() {
    await API.setKasumiUnameMode(forms.unameMode);
    await API.clearKasumiUname(forms.unameMode);
  }

  onMount(() => {
    void initialize();
  });

  const status = createMemo(() => kasumiStore.status);
  const config = createMemo(() => status()?.config);
  const lkm = createMemo(() => status()?.lkm);
  const activeModules = createMemo(
    () => status()?.runtime?.kasumi_modules || [],
  );
  const mapsSpoofSupported = createMemo(() =>
    (status()?.feature_names || []).includes("maps_spoof"),
  );
  const kmiOptions = createMemo(() => {
    const options = ["", ...KNOWN_KMI_OPTIONS];
    if (forms.kmi && !options.includes(forms.kmi)) {
      options.push(forms.kmi);
    }
    return options;
  });
  const heroStatusText = createMemo(() => {
    if (loading()) {
      return uiStore.L.kasumi?.statusLoading ?? "Loading";
    }
    if (status()?.status === "disabled_runtime_present") {
      return (
        uiStore.L.kasumi?.statusConfigOffRuntimeOn ??
        "Config Off / Runtime Still Loaded"
      );
    }
    if (status()?.status === "disabled") {
      return uiStore.L.kasumi?.statusDisabled ?? "Disabled";
    }
    return status()?.available
      ? (uiStore.L.kasumi?.statusWorking ?? "Working")
      : (uiStore.L.kasumi?.statusUnavailable ?? "Unavailable");
  });
  const heroSubtitleText = createMemo(
    () =>
      `API ${status()?.protocol_version ?? "-"} · ${uiStore.L.kasumi?.rulesBadge ?? "Rules"} ${status()?.rule_count ?? 0}`,
  );
  const statusChipText = createMemo(
    () => status()?.mirror_path || config()?.mirror_path || "-",
  );
  const unameModeDescription = createMemo(() =>
    forms.unameMode === "global"
      ? (uiStore.L.kasumi?.unameModeGlobalDesc ??
        "System-wide: rewrites init_uts_ns, every task sees fake values.")
      : (uiStore.L.kasumi?.unameModeScopedDesc ??
        "Per-process: only Kasumi-hidden UIDs see fake values."),
  );

  function isSectionExpanded(id: string) {
    return expandedSection() === id;
  }

  async function handleSectionToggle(id: string) {
    const willExpand = expandedSection() !== id;
    setExpandedSection(willExpand ? id : null);

    if (!willExpand || id !== "user-hide") return;

    try {
      await loadUserHideRules();
    } catch (e: unknown) {
      uiStore.showToast(
        getErrorMessage(
          e,
          uiStore.L.kasumi?.loadError ?? "Failed to load Kasumi",
        ),
        "error",
      );
    }
  }

  return (
    <>
      <div class="dialog-container">
        <md-dialog
          open={showKmiDialog()}
          onclose={() => setShowKmiDialog(false)}
          class="transparent-scrim"
        >
          <div slot="headline">
            {uiStore.L.kasumi?.kmiOverride ?? "KMI Override"}
          </div>
          <div slot="content" class="kasumi-kmi-dialog">
            <md-list>
              <For each={kmiOptions()}>
                {(option) => {
                  const label = option
                    ? option
                    : (uiStore.L.kasumi?.autoKmi ?? "Auto Detect");

                  return (
                    <md-list-item
                      class="lang-option"
                      type="button"
                      onClick={() => {
                        setForms("kmi", option);
                        setShowKmiDialog(false);
                      }}
                    >
                      <div slot="headline">{label}</div>
                      <Show when={forms.kmi === option}>
                        <md-icon slot="end">
                          <svg viewBox="0 0 24 24">
                            <path d={ICONS.check} />
                          </svg>
                        </md-icon>
                      </Show>
                    </md-list-item>
                  );
                }}
              </For>
            </md-list>
          </div>
          <div slot="actions">
            <md-text-button onClick={() => setShowKmiDialog(false)}>
              {uiStore.L.common?.cancel ?? "Cancel"}
            </md-text-button>
          </div>
        </md-dialog>
      </div>

      <div class="dialog-container">
        <md-dialog
          open={showUnloadLkmWarning()}
          onclose={() => setShowUnloadLkmWarning(false)}
          class="transparent-scrim"
        >
          <div slot="headline">
            {uiStore.L.kasumi?.unloadLkmWarningTitle ?? "注意！"}
          </div>
          <div slot="content">
            {uiStore.L.kasumi?.unloadLkmWarningBody ??
              "Kasumi 使用 TSR 模式时可能无法卸载，如 5 秒内未成功卸载，Kasumi 将会在 3 秒后自动安全重启，你确定要卸载吗？"}
          </div>
          <div slot="actions">
            <md-text-button onClick={() => setShowUnloadLkmWarning(false)}>
              {uiStore.L.kasumi?.unloadLkmWarningCancel ?? "取消"}
            </md-text-button>
            <md-text-button
              onClick={() => {
                setShowUnloadLkmWarning(false);
                void runAction(
                  () => API.unloadKasumiLkm(),
                  uiStore.L.kasumi?.unloadLkm ?? "LKM unloaded",
                );
              }}
            >
              {uiStore.L.kasumi?.unloadLkmWarningConfirm ?? "确定"}
            </md-text-button>
          </div>
        </md-dialog>
      </div>

      <div class="kasumi-page">
        <div class="dashboard-grid kasumi-dashboard-grid">
          <HeroCard
            loading={loading()}
            heroStatusText={heroStatusText()}
            heroSubtitleText={heroSubtitleText()}
            statusChipText={statusChipText()}
          />
        </div>

        <div class="kasumi-grid">
          <LkmSection
            pending={pending()}
            kmi={forms.kmi}
            setKmi={(v: string) => setForms("kmi", v)}
            lkm={lkm()}
            isExpanded={isSectionExpanded("lkm")}
            onToggle={() => void handleSectionToggle("lkm")}
            runAction={runAction}
            onShowKmiDialog={() => setShowKmiDialog(true)}
            onShowUnloadWarning={() => setShowUnloadLkmWarning(true)}
          />

          <RuntimeSection
            pending={pending()}
            config={config()}
            status={status()}
            lkm={lkm()}
            isExpanded={isSectionExpanded("runtime")}
            onToggle={() => void handleSectionToggle("runtime")}
            runAction={runAction}
          />

          <IdentitySection
            pending={pending()}
            unameMode={forms.unameMode}
            setUnameMode={(v: "scoped" | "global") => setForms("unameMode", v)}
            release={forms.release}
            setRelease={(v: string) => setForms("release", v)}
            version={forms.version}
            setVersion={(v: string) => setForms("version", v)}
            cmdline={forms.cmdline}
            setCmdline={(v: string) => setForms("cmdline", v)}
            unameModeDescription={unameModeDescription()}
            isExpanded={isSectionExpanded("identity")}
            onToggle={() => void handleSectionToggle("identity")}
            runAction={runAction}
            fillOriginalKernelUname={fillOriginalKernelUname}
            saveAndApplyUname={saveAndApplyUname}
            clearUname={clearUname}
          />

          <UserHideSection
            pending={pending()}
            userHidePath={forms.userHidePath}
            setUserHidePath={(v: string) => setForms("userHidePath", v)}
            userHideRules={userHideRules()}
            isExpanded={isSectionExpanded("user-hide")}
            onToggle={() => void handleSectionToggle("user-hide")}
            runAction={runAction}
          />

          <Show when={status()?.available && mapsSpoofSupported()}>
            <MapsSection
              pending={pending()}
              mapsTargetIno={forms.mapsTargetIno}
              setMapsTargetIno={(v: string) => setForms("mapsTargetIno", v)}
              mapsTargetDev={forms.mapsTargetDev}
              setMapsTargetDev={(v: string) => setForms("mapsTargetDev", v)}
              mapsSpoofedIno={forms.mapsSpoofedIno}
              setMapsSpoofedIno={(v: string) => setForms("mapsSpoofedIno", v)}
              mapsSpoofedDev={forms.mapsSpoofedDev}
              setMapsSpoofedDev={(v: string) => setForms("mapsSpoofedDev", v)}
              mapsPath={forms.mapsPath}
              setMapsPath={(v: string) => setForms("mapsPath", v)}
              config={config()}
              isExpanded={isSectionExpanded("maps")}
              onToggle={() => void handleSectionToggle("maps")}
              runAction={runAction}
            />
          </Show>

          <FeaturesSection
            loading={loading()}
            status={status()}
            config={config()}
            activeModules={activeModules()}
            isExpanded={isSectionExpanded("features")}
            onToggle={() => void handleSectionToggle("features")}
          />
        </div>
      </div>

      <BottomActions>
        <md-filled-tonal-icon-button
          disabled={pending()}
          onClick={() => load("full")}
          title={uiStore.L.kasumi?.refresh ?? "Refresh"}
        >
          <md-icon>
            <svg viewBox="0 0 24 24">
              <path d={ICONS.refresh} />
            </svg>
          </md-icon>
        </md-filled-tonal-icon-button>
        <div class="spacer" />
        <md-filled-button
          disabled={pending()}
          onClick={() =>
            runAction(
              () => API.clearKasumiRules(),
              uiStore.L.kasumi?.clearRules ?? "Rules cleared",
            )
          }
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.delete} />
            </svg>
          </md-icon>
          {uiStore.L.kasumi?.clearRules ?? "Clear Rules"}
        </md-filled-button>
      </BottomActions>
    </>
  );
}
