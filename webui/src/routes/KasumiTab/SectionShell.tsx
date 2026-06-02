import type { SectionShellProps } from "./types";

const EXPAND_MORE_ICON = "M7.41 8.59 12 13.17l4.59-4.58L18 10l-6 6-6-6z";

export default function SectionShell(props: SectionShellProps) {
  return (
    <section
      class={`kasumi-card kasumi-section ${props.isExpanded ? "expanded" : ""}`}
    >
      <button
        class="kasumi-section-toggle"
        type="button"
        aria-expanded={props.isExpanded ? "true" : "false"}
        aria-controls={`kasumi-section-${props.id}`}
        onClick={props.onToggle}
      >
        <div class="kasumi-card-head kasumi-section-toggle-inner">
          <div>
            <div class="kasumi-card-title">{props.title}</div>
          </div>
          <div class="kasumi-section-toggle-end">
            {props.badge && (
              <div class={`state-pill ${props.badgeActive ? "active" : ""}`}>
                {props.badge}
              </div>
            )}
            <md-icon class="kasumi-section-chevron" aria-hidden="true">
              <svg viewBox="0 0 24 24">
                <path d={EXPAND_MORE_ICON} />
              </svg>
            </md-icon>
          </div>
        </div>
      </button>
      <div
        class="kasumi-section-body-wrapper"
        id={`kasumi-section-${props.id}`}
      >
        <div class="kasumi-section-body-inner">
          <div class="kasumi-section-body">{props.children}</div>
        </div>
      </div>
    </section>
  );
}
