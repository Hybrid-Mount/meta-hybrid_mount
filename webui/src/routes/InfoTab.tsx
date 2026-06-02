import { createSignal, Show, For, onMount } from "solid-js";
import { uiStore } from "../lib/stores/uiStore";
import { sysStore } from "../lib/stores/sysStore";
import { openLink } from "../lib/api/services/systemService";
import { ICONS } from "../lib/constants";
import { IS_RELEASE } from "../lib/constants_gen";
import { INFO_TAB_SECTIONS } from "../lib/infoTabData.gen";
import "./InfoTab.css";
import "@material/web/button/filled-tonal-button.js";
import "@material/web/button/text-button.js";
import "@material/web/dialog/dialog.js";
import "@material/web/icon/icon.js";
import "@material/web/list/list.js";
import "@material/web/list/list-item.js";

const PRIMARY_REPO_OWNER = "Hybrid-Mount";
const PRIMARY_REPO_NAME = "meta-hybrid_mount";
const TELEGRAM_LINK = "https://t.me/hybridmountchat";
const PAYPAL_LINK = "https://www.paypal.me/LangQin280";

interface MdDialogElement extends HTMLElement {
  show: () => void;
  close: () => void;
}

export default function InfoTab() {
  const [activeQr, setActiveQr] = createSignal<string>("");

  onMount(() => {
    void sysStore.ensureVersionLoaded();
  });

  let donateDialogRef: HTMLElement | undefined;
  let qrDialogRef: HTMLElement | undefined;

  const isDev = () => !IS_RELEASE;

  function handleLink(e: MouseEvent, url: string) {
    e.preventDefault();

    void openLink(url).catch(() => {
      window.open(url, "_blank", "noopener,noreferrer");
    });
  }

  function openDonate(e: MouseEvent) {
    e.preventDefault();
    (donateDialogRef as MdDialogElement)?.show();
  }

  function closeDonate() {
    (donateDialogRef as MdDialogElement)?.close();
  }

  function openQr(path: string) {
    setActiveQr(path);
    (qrDialogRef as MdDialogElement)?.show();
  }

  function closeQr() {
    (qrDialogRef as MdDialogElement)?.close();
  }

  return (
    <div class="info-container">
      <div class="project-header">
        <div class="app-logo">
          <Show
            when={!isDev()}
            fallback={
              <svg
                xmlns="http://www.w3.org/2000/svg"
                viewBox="0 0 120 120"
                class="dev-logo"
              >
                <circle cx="60" cy="60" r="50" class="logo-base-track" />
                <circle cx="60" cy="60" r="38" class="logo-base-track" />
                <circle cx="60" cy="60" r="26" class="logo-base-track" />

                <g class="dev-logo-outer-group">
                  <path
                    d="M 60 10 A 50 50 0 1 1 10 60"
                    class="logo-arc logo-arc-outer"
                  />
                </g>

                <g class="dev-logo-mid-group">
                  <path
                    d="M 60 22 A 38 38 0 0 1 60 98"
                    class="logo-arc logo-arc-mid logo-arc-error"
                  />
                </g>

                <g class="dev-logo-inner-group">
                  <path
                    d="M 60 34 A 26 26 0 1 1 47 82.5"
                    class="logo-arc logo-arc-inner"
                  />
                </g>

                <circle cx="60" cy="60" r="10" class="logo-core" />
              </svg>
            }
          >
            <svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 120 120">
              <circle cx="60" cy="60" r="50" class="logo-base-track" />
              <circle cx="60" cy="60" r="38" class="logo-base-track" />
              <circle cx="60" cy="60" r="26" class="logo-base-track" />

              <path
                d="M60 10 A 50 50 0 0 1 110 60"
                class="logo-arc logo-arc-outer"
              />
              <path
                d="M60 98 A 38 38 0 0 1 60 22"
                class="logo-arc logo-arc-mid"
              />
              <path
                d="M34 60 A 26 26 0 1 1 86 60"
                class="logo-arc logo-arc-inner"
              />

              <circle cx="60" cy="60" r="10" class="logo-core" />
            </svg>
          </Show>
        </div>
        <span class="app-name">{uiStore.L.common.appName}</span>
        <span class="app-version">{sysStore.version}</span>
      </div>

      <div class="action-buttons">
        <md-filled-tonal-button
          class="action-btn"
          onClick={(e: MouseEvent) =>
            handleLink(
              e,
              `https://github.com/${PRIMARY_REPO_OWNER}/${PRIMARY_REPO_NAME}`,
            )
          }
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.github} />
            </svg>
          </md-icon>
          {uiStore.L.info.projectLink}
        </md-filled-tonal-button>

        <md-filled-tonal-button
          class="action-btn"
          onClick={(e: MouseEvent) => handleLink(e, TELEGRAM_LINK)}
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.telegram} />
            </svg>
          </md-icon>
          {uiStore.L.info?.telegram ?? "Telegram"}
        </md-filled-tonal-button>

        <md-filled-tonal-button
          class="action-btn donate-btn"
          onClick={openDonate}
        >
          <md-icon slot="icon">
            <svg viewBox="0 0 24 24">
              <path d={ICONS.donate} />
            </svg>
          </md-icon>
          {uiStore.L.info.donate}
        </md-filled-tonal-button>
      </div>

      <div class="contributors-section">
        <div class="section-title">{uiStore.L.info.contributors}</div>

        <div class="contributors-groups">
          <For each={INFO_TAB_SECTIONS}>
            {(section) => (
              <div class="contributor-group">
                <div class="group-header">
                  <button
                    class="group-link"
                    onClick={(e: MouseEvent) => handleLink(e, section.repoUrl)}
                    type="button"
                  >
                    <div class="group-title">{section.label}</div>
                    <div class="group-subtitle">{section.repoDisplayName}</div>
                  </button>
                </div>

                <div class="list-wrapper">
                  <md-list class="contributors-list">
                    <For each={section.contributors}>
                      {(user) => (
                        <md-list-item
                          class="contributor-link"
                          type="link"
                          href={user.html_url}
                          target="_blank"
                          onClick={(e: MouseEvent) =>
                            handleLink(e, user.html_url)
                          }
                        >
                          <img
                            slot="start"
                            src={`${user.avatar_url}${user.avatar_url.includes("?") ? "&" : "?"}s=80`}
                            alt={user.login}
                            class="c-avatar"
                            loading="lazy"
                          />
                          <div slot="headline">{user.name || user.login}</div>
                          <div slot="supporting-text">
                            {user.bio || uiStore.L.info.noBio}
                          </div>
                        </md-list-item>
                      )}
                    </For>
                  </md-list>
                </div>
              </div>
            )}
          </For>
        </div>
      </div>

      <md-dialog ref={donateDialogRef} class="donate-dialog">
        <div slot="headline">{uiStore.L.info?.supportUs ?? "Support Us"}</div>
        <div slot="content" class="donate-content">
          <div class="donate-section">
            <div class="author-label">
              {uiStore.L.info?.authorYuzaki ?? "YuzakiKokuban"}
            </div>
            <div class="donate-grid">
              <md-filled-tonal-button
                onClick={() => openQr("/assets/donate/yuzaki_alipay.jpg")}
              >
                Alipay
              </md-filled-tonal-button>
              <md-filled-tonal-button
                onClick={() => openQr("/assets/donate/yuzaki_wechat.jpg")}
              >
                WeChat
              </md-filled-tonal-button>
              <md-filled-tonal-button
                onClick={() => openQr("/assets/donate/yuzaki_binance.jpg")}
              >
                Binance
              </md-filled-tonal-button>
              <md-filled-tonal-button
                onClick={(e: MouseEvent) => handleLink(e, PAYPAL_LINK)}
              >
                <md-icon slot="icon">
                  <svg viewBox="0 0 24 24">
                    <path d={ICONS.donate} />
                  </svg>
                </md-icon>
                PayPal
              </md-filled-tonal-button>
            </div>
          </div>

          <div class="donate-divider"></div>

          <div class="donate-section">
            <div class="author-label">Tools-cx-app</div>
            <div class="donate-grid">
              <md-filled-tonal-button
                onClick={() => openQr("/assets/donate/tools_wechat.jpg")}
              >
                WeChat
              </md-filled-tonal-button>
            </div>
          </div>
        </div>
        <div slot="actions">
          <md-text-button onClick={closeDonate}>
            {uiStore.L.common?.close ?? "Close"}
          </md-text-button>
        </div>
      </md-dialog>

      <md-dialog ref={qrDialogRef} class="qr-dialog" onClick={closeQr}>
        <div slot="content" class="qr-content-wrapper">
          <Show when={activeQr()}>
            <img src={activeQr()} alt="Scan QR Code" />
          </Show>
        </div>
      </md-dialog>
    </div>
  );
}
