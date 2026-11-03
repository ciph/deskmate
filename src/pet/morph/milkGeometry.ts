import type { Companion } from "./engine";

/** Separate the milk body's underside from its feet; the classic model stays intact. */
export function milkOutline(engine: Companion, includeFeet = false): number[] {
  const outline = [...engine.model.outline];
  const solid = 1 - engine.model.ghost;
  for (let i = 1; i < outline.length; i += 2) {
    const crown = Math.max(0, Math.min(1, (-outline[i] - 40) / 100));
    outline[i - 1] *= 1 - 0.06 * crown * solid;
    if (i >= 31 && i <= 67) {
      if (includeFeet) continue;
      const underside = -7 - 2 * Math.min(1, Math.abs(outline[i - 1]) / 100);
      outline[i] += (underside - outline[i]) * solid;
    } else {
      // A fuller crown and rounded lower corners, without changing morph topology.
      outline[i] -= 6 * Math.max(0, -outline[i] / 175) * solid;
    }
  }
  return outline;
}

export function paintMilkBody(body: SVGPathElement | null, feet: SVGGElement | null, engine: Companion) {
  const outline = milkOutline(engine);
  const points: [number, number][] = [];
  for (let i = 0; i < outline.length; i += 2) points.push(engine.deform(outline[i], outline[i + 1]));
  let d = `M${points[0].join(" ")}`;
  for (let i = 1; i < points.length; i += 3) d += ` C${points.slice(i, i + 3).flat().join(" ")}`;
  body?.setAttribute("d", `${d} Z`);
  feet?.setAttribute("opacity", String(1 - engine.model.ghost));
  [36, 60].forEach((offset, i) => {
    const a = engine.model.outline;
    const point = engine.deform(a[offset], a[offset + 1] - 14);
    feet?.children[i]?.setAttribute("transform", `translate(${point.join(" ")}) scale(${engine.p.sx} ${engine.p.sy})`);
  });
}
