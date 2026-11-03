import { useEffect, useId, useMemo, useRef, useState, type KeyboardEvent } from "react";
import { AppIcon } from "../../ui/AppIcon";
import { sector, visibleArc, type RadialItem, type RadialModel } from "./model";
import "../../theme.css";
import "./radialMenu.css";

export function RadialMenu({ model, onFinish }: { readonly model: RadialModel; readonly onFinish: (action: string | null) => void }) {
  const [page, setPage] = useState<string | null>(null);
  const parent = model.items.find(item => item.id === page && item.enabled);
  const choices = parent?.children ?? model.items;
  const zh = model.language.startsWith("zh");
  const root = useRef<HTMLDivElement>(null);
  const returnTo = useRef<string | null>(null);
  const clipId = useId().replaceAll(":", "");
  const layout = model.layout;
  const arc = useMemo(() => layout ? visibleArc(layout) : null, [layout]);
  const items: readonly RadialItem[] = [...choices, { id: "menu-back", label: parent ? (zh ? "返回" : "Back") : (zh ? "关闭" : "Close"), icon: parent ? "back" : "close", enabled: true }];
  useEffect(() => {
    const buttons = root.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)");
    const restored = Array.from(buttons ?? []).find(button => button.dataset.item === returnTo.current);
    (restored ?? buttons?.[0])?.focus();
    returnTo.current = null;
  }, [parent?.id]);
  const back = () => { if (parent) { returnTo.current = parent.id; setPage(null); } else onFinish(null); };
  const activate = (item: RadialItem) => { if (item.children) setPage(item.id); else onFinish(item.id); };
  const keyDown = (event: KeyboardEvent) => {
    if (event.key === "Escape") { event.preventDefault(); onFinish(null); return; }
    if (event.key === "Backspace") { event.preventDefault(); back(); return; }
    const keys = ["Tab", "ArrowRight", "ArrowDown", "ArrowLeft", "ArrowUp", "Home", "End"];
    if (!keys.includes(event.key)) return;
    event.preventDefault();
    const buttons = Array.from(root.current?.querySelectorAll<HTMLButtonElement>("button:not(:disabled)") ?? []);
    const current = buttons.indexOf(document.activeElement as HTMLButtonElement);
    const direction = event.key === "ArrowLeft" || event.key === "ArrowUp" || (event.key === "Tab" && event.shiftKey) ? -1 : 1;
    const index = event.key === "Home" ? 0 : event.key === "End" ? buttons.length - 1 : (current + direction + buttons.length) % buttons.length;
    buttons[index]?.focus();
  };
  if (!layout || !arc) return null;
  return <div className="pet-menu-root" data-theme={model.theme}>
    <div className="radial-dismiss" onPointerDown={() => onFinish(null)} aria-hidden="true" />
    <div ref={root} className="radial-menu" style={{ transformOrigin: `${layout.centerX}px ${layout.centerY}px` }} role="dialog" aria-modal="true" aria-label={`${model.title} ${zh ? "扇形菜单" : "radial menu"}`} onKeyDown={keyDown} onContextMenu={event => { event.preventDefault(); onFinish(null); }}>
      <svg className="radial-definitions" aria-hidden="true"><defs><linearGradient id={`${clipId}-rim`} x1="0" y1="0" x2=".65" y2="1"><stop stopColor="white" stopOpacity=".8" /><stop offset=".5" stopColor="white" stopOpacity=".16" /><stop offset="1" stopColor="currentColor" stopOpacity=".18" /></linearGradient>{items.map((item, index) => <clipPath key={item.id} id={`${clipId}-${index}`} clipPathUnits="objectBoundingBox"><path d={sector(index, items.length, layout, arc).path} /></clipPath>)}</defs></svg>
      <div className="radial-page" key={parent?.id ?? "home"}>
        {items.map((item, index) => {
          const geometry = sector(index, items.length, layout, arc);
          return <button key={item.id} type="button" className={`radial-sector${item.id === "menu-back" ? " radial-back" : ""}`} data-item={item.id} disabled={!item.enabled}
            aria-label={item.label} aria-pressed={item.checked}
            style={{ clipPath: `url(#${clipId}-${index})` }} onClick={() => item.id === "menu-back" ? back() : activate(item)}>
            <svg className="radial-rim" viewBox="0 0 1 1" preserveAspectRatio="none" aria-hidden="true"><path d={geometry.path} fill="none" stroke={`url(#${clipId}-rim)`} strokeWidth="2" vectorEffect="non-scaling-stroke" /></svg>
            <span className="radial-label" style={{ left: geometry.label[0], top: geometry.label[1] }}>
              <AppIcon name={item.icon} size={20} /><span>{item.label}</span>
              {(item.checked || item.children) && <span className="radial-marker" aria-hidden="true">{item.checked ? "✓" : "›"}</span>}
            </span>
          </button>;
        })}
      </div>

    </div>
  </div>;
}
