import type { AppIconName } from "../../ui/AppIcon";
import type { GifRect } from "../gifGeometry";

export interface RadialLayout {
  readonly width: number; readonly height: number;
  readonly centerX: number; readonly centerY: number;
  readonly innerRadius: number; readonly outerRadius: number;
}

export interface RadialItem {
  readonly id: string;
  readonly label: string;
  readonly icon: AppIconName;
  readonly enabled: boolean;
  readonly checked?: boolean;
  readonly children?: readonly RadialItem[];
}
export interface RadialModel {
  readonly id: string;
  readonly revision: number;
  readonly title: string;
  readonly theme: string;
  readonly language: string;
  readonly anchor: GifRect | null;
  readonly layout?: RadialLayout;
  readonly items: readonly RadialItem[];
  readonly usage?: UsageCloud;
}
export interface UsageCloud {
  readonly label: string;
  readonly percent: number | null;
  readonly amount: string;
  readonly note: string;
}
export const MENU_UPDATE = "deskmate://pet-menu-update";
export const MENU_RESULT = "deskmate://pet-menu-result";
export interface MenuResult { readonly id: string; readonly action: string | null }

export function findAction(items: readonly RadialItem[], id: string): RadialItem | undefined {
  for (const item of items) {
    if (!item.enabled) continue;
    if (item.children) { const child = findAction(item.children, id); if (child) return child; }
    else if (item.id === id) return item;
  }
}

/** Keep every sector inside the visible work area without moving the pet out of its center. */
export function visibleArc(layout: RadialLayout) {
  const steps = 720, step = Math.PI * 2 / steps;
  const fits = (angle: number) => [layout.innerRadius, layout.outerRadius + 6].every(radius => {
    const x = layout.centerX + radius * Math.cos(angle), y = layout.centerY + radius * Math.sin(angle);
    return x >= 0 && x <= layout.width && y >= 0 && y <= layout.height;
  });
  const visible = Array.from({ length: steps }, (_, i) => fits(i * step));
  if (visible.every(Boolean)) return { start: -Math.PI / 2, sweep: Math.PI * 2 };
  let longest = 0, start = 0, run = 0;
  for (let i = 0; i < steps * 2; i++) {
    run = visible[i % steps] ? run + 1 : 0;
    if (run > longest) { longest = run; start = i - run + 1; }
  }
  return { start: (start % steps) * step, sweep: Math.max(0, longest - 1) * step };
}

/** Object-bounding-box coordinates keep paint and hit testing aligned in a clipped native window. */
export function sector(index: number, count: number, layout: RadialLayout, arc = visibleArc(layout)) {
  const point = (r: number, angle: number) => [layout.centerX + r * Math.cos(angle), layout.centerY + r * Math.sin(angle)] as const;
  const normalized = (r: number, angle: number) => { const [x, y] = point(r, angle); return [x / layout.width, y / layout.height]; };
  const step = arc.sweep / count;
  const offset = arc.sweep === Math.PI * 2 ? -step / 2 : 0;
  const start = arc.start + offset + index * step + .015, end = arc.start + offset + (index + 1) * step - .015;
  const outer = layout.outerRadius, inner = layout.innerRadius;
  const halfSweepSin = Math.sin(Math.min(Math.PI, end - start) / 2);
  const corner = Math.min(8, (outer - inner) / 2, outer * halfSweepSin / (1 + halfSweepSin), inner * halfSweepSin / (1 - halfSweepSin));
  // Circular fillets are tangent to both the radial edges and the inner/outer arcs.
  const outerInset = Math.asin(corner / (outer - corner)), innerInset = Math.asin(corner / (inner + corner));
  const outerEdge = Math.sqrt(outer * (outer - 2 * corner)), innerEdge = Math.sqrt(inner * (inner + 2 * corner));
  const round = `A ${corner / layout.width} ${corner / layout.height} 0 0 1`;
  const path = [
    `M ${normalized(outer, start + outerInset)}`,
    `A ${outer / layout.width} ${outer / layout.height} 0 ${end - start - 2 * outerInset > Math.PI ? 1 : 0} 1 ${normalized(outer, end - outerInset)}`,
    `${round} ${normalized(outerEdge, end)}`,
    `L ${normalized(innerEdge, end)}`,
    `${round} ${normalized(inner, end - innerInset)}`,
    `A ${inner / layout.width} ${inner / layout.height} 0 ${end - start - 2 * innerInset > Math.PI ? 1 : 0} 0 ${normalized(inner, start + innerInset)}`,
    `${round} ${normalized(innerEdge, start)}`,
    `L ${normalized(outerEdge, start)}`,
    `${round} ${normalized(outer, start + outerInset)} Z`,
  ].join(" ");
  return { path, label: point((inner + outer) / 2, (start + end) / 2), start, end };
}
