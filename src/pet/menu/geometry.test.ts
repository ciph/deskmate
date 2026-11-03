import { expect, test } from "bun:test";
import { petContentBounds, setGifHitPolygon } from "../gifGeometry";
import { sector, visibleArc, type RadialLayout } from "./model";

test("menu anchor follows the visible silhouette rather than the transparent frame", () => {
  const root = document.createElement("div"), image = document.createElement("span");
  image.className = "gif-pet-image"; root.append(image);
  image.getBoundingClientRect = () => new DOMRect(20, 40, 480, 480);
  setGifHitPolygon(image, [[80, 100], [160, 100], [160, 180], [80, 180]]);
  expect(petContentBounds(root)).toEqual({ left: 180, top: 240, width: 160, height: 160 });
  image.dataset.hitDisabled = "true"; expect(petContentBounds(root)).toBeNull();
});

test("full circles and clipped fans keep every sector and label visible while clearing the pet", () => {
  const layouts: RadialLayout[] = [
    { width: 368, height: 368, centerX: 184, centerY: 184, innerRadius: 110, outerRadius: 174 },
    { width: 368, height: 264, centerX: 184, centerY: 184, innerRadius: 110, outerRadius: 174 },
    { width: 274, height: 264, centerX: 90, centerY: 184, innerRadius: 110, outerRadius: 174 },
    { width: 274, height: 264, centerX: 184, centerY: 184, innerRadius: 110, outerRadius: 174 },
  ];
  expect(visibleArc(layouts[0]).sweep).toBe(Math.PI * 2);
  expect(visibleArc(layouts[1]).sweep).toBeLessThan(Math.PI * 2);
  for (const layout of layouts) for (const count of [3, 5, 7]) {
    const arc = visibleArc(layout);
    expect(arc.sweep).toBeGreaterThan(Math.PI / 2);
    for (let i = 0; i < count; i++) {
      const wedge = sector(i, count, layout, arc);
      expect(wedge.path).not.toContain("NaN");
      expect(wedge.label[0]).toBeGreaterThan(30); expect(wedge.label[0]).toBeLessThan(layout.width - 30);
      expect(wedge.label[1]).toBeGreaterThan(28); expect(wedge.label[1]).toBeLessThan(layout.height - 28);
      for (let step = 0; step <= 16; step++) {
        const angle = wedge.start + (wedge.end - wedge.start) * step / 16;
        for (const radius of [layout.innerRadius, layout.outerRadius]) {
          const x = layout.centerX + Math.cos(angle) * radius, y = layout.centerY + Math.sin(angle) * radius;
          expect(x).toBeGreaterThanOrEqual(0); expect(x).toBeLessThanOrEqual(layout.width);
          expect(y).toBeGreaterThanOrEqual(0); expect(y).toBeLessThanOrEqual(layout.height);
        }
      }
    }
  }
});
