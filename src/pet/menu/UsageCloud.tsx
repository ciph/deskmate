import { useEffect, useLayoutEffect, useRef, useState, type CSSProperties } from "react";
import { dict } from "../../lib/i18n";
import type { RadialModel, UsageCloud as UsageCloudData } from "./model";
import "../../theme.css";
import "./usageCloud.css";

export function UsageCloud({ model, usage, onClose, visible = true }: {
  readonly model: RadialModel; readonly usage: UsageCloudData; readonly onClose: () => void; readonly visible?: boolean;
}) {
  const root = useRef<HTMLDivElement>(null);
  const button = useRef<HTMLButtonElement>(null);
  const [fadeFrom, setFadeFrom] = useState<string | null>(null);
  const closing = fadeFrom !== null;
  const closed = useRef(false);
  const close = useRef(onClose);
  useLayoutEffect(() => { close.current = onClose; }, [onClose]);
  const t = dict(model.language);
  const finish = () => { if (!closed.current) { closed.current = true; close.current(); } };
  const dismiss = () => {
    if (closing) return;
    setFadeFrom(root.current ? getComputedStyle(root.current).opacity || "1" : "1");
  };
  useEffect(() => { button.current?.focus(); }, []);
  useEffect(() => {
    if (!closing) return;
    // A hidden WebView or reduced-motion preference can suppress animationend.
    const timer = window.setTimeout(finish, window.matchMedia("(prefers-reduced-motion: reduce)").matches ? 0 : 400);
    return () => window.clearTimeout(timer);
  }, [closing]);
  return <div ref={root} className="pet-menu-root pet-usage-root" data-theme={model.theme}
    data-phase={closing ? "leaving" : visible ? "visible" : "waiting"}
    style={{ "--pet-usage-fade-from": fadeFrom ?? "1" } as CSSProperties}
    onAnimationEnd={event => {
      if (event.target === event.currentTarget && event.animationName === "pet-usage-fade-out" && closing) finish();
    }}>
    <button ref={button} type="button" className="pet-usage-cloud" onClick={dismiss} aria-disabled={closing}
      aria-label={`${t.aiUsageTitle} · ${t.petAiUsageClose}`} aria-describedby="pet-usage-values"
      onKeyDown={event => { if (event.key === "Escape") { event.preventDefault(); dismiss(); } }}>
      <svg className="pet-usage-outline" viewBox="0 0 248 172" preserveAspectRatio="none" aria-hidden="true">
        <path d="M44 137C20 137 8 122 8 101C8 87 15 76 27 69C24 49 39 34 61 34C70 9 107 6 125 25C150 5 186 18 190 41C215 36 236 55 233 79C250 101 234 137 208 137Z" />
        <ellipse cx="132" cy="148" rx="10" ry="6" /><ellipse cx="124" cy="163" rx="5" ry="4" />
      </svg>
      <span className="pet-usage-content" id="pet-usage-values" role="status">
        <span className="pet-usage-title">{t.aiUsageTitle}{usage.label && ` · ${usage.label}`}</span>
        <span className="pet-usage-percent">{usage.percent === null ? "—" : `${usage.percent}%`}</span>
        <span className="pet-usage-amount">{usage.amount}</span>
        {usage.note && <span className="pet-usage-note">{usage.note}</span>}
      </span>
    </button>
  </div>;
}
