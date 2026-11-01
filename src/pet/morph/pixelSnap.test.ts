import { expect, test } from "bun:test";
import { GLASSES_CELL, glassesTransform } from "./pixelSnap";

const parse = (transform: string) => {
  const [, x, y, s] = /translate\((\S+) (\S+)\) scale\((\S+)\)/.exec(transform)!;
  return { x: Number(x), y: Number(y), s: Number(s) };
};

for (const ratio of [1, 1.25, 1.5, 1.75, 2])
  for (const width of [80, 160, 213.3, 320])
    test(`glasses cells land on whole device pixels at ${ratio * 100}% for a ${width}px pet`, () => {
      const scale = ((width / 270) * 360) / 360;
      for (const origin of [{ x: 0, y: 0 }, { x: 160.4, y: 233.37 }, { x: 77.1, y: 12.9 }])
        for (const face of [[0, -100], [3.217, -93.881], [-7.5, -121.04]] as const) {
          const grid = { scale, ratio, ...origin };
          const t = parse(glassesTransform(grid, face));
          const unit = scale * ratio;
          const cell = GLASSES_CELL * t.s * unit;
          expect(Math.abs(cell - Math.round(cell))).toBeLessThan(1e-3);
          expect(Math.round(cell)).toBeGreaterThanOrEqual(1);
          // Glasses origin (local 0,0) in device pixels after face + snap translation.
          const dx = (origin.x + (face[0] + t.x) * scale) * ratio;
          const dy = (origin.y + (face[1] + t.y) * scale) * ratio;
          expect(Math.abs(dx - Math.round(dx))).toBeLessThan(2e-3);
          expect(Math.abs(dy - Math.round(dy))).toBeLessThan(2e-3);
          // The nudge never exceeds half a device pixel; size changes stay small.
          expect(Math.abs(t.x * unit)).toBeLessThanOrEqual(0.5 + 1e-3);
          expect(Math.abs(t.y * unit)).toBeLessThanOrEqual(0.5 + 1e-3);
          if (GLASSES_CELL * unit >= 4) expect(Math.abs(t.s - 1)).toBeLessThan(0.13);
        }
    });

test("missing geometry leaves the glasses untouched", () => {
  expect(glassesTransform(null, [0, -100])).toBe("");
  expect(glassesTransform({ scale: 0, ratio: 1, x: 0, y: 0 }, [0, -100])).toBe("");
});
