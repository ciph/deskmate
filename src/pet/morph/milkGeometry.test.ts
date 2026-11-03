import { expect, test } from "bun:test";
import { Companion, FORM_KEYS } from "./engine";
import { milkOutline, paintMilkBody } from "./milkGeometry";

test("milk geometry leaves the classic model intact and joins the ghost without a silhouette jump", () => {
  const engine = new Companion({ auto: false });
  const original = structuredClone(engine.model);
  const solid = milkOutline(engine);
  expect(engine.model).toEqual(original);
  // The original foot bulges must no longer protrude from the body surface.
  expect(solid[37]).toBeLessThan(-7);
  expect(solid[61]).toBeLessThan(-7);
  engine.selectForm("ghost");
  let last = solid;
  for (let i = 0; i < 120; i++) {
    engine.update(1 / 120);
    const next = milkOutline(engine);
    expect(Math.max(...next.map((value, j) => Math.abs(value - last[j])))).toBeLessThan(4);
    last = next;
  }
  expect(milkOutline(engine)).toEqual(engine.model.outline);
});

test("feet stay attached and body coordinates stay finite through every form and movement", () => {
  const body = document.createElementNS("http://www.w3.org/2000/svg", "path");
  const feet = document.createElementNS("http://www.w3.org/2000/svg", "g");
  feet.innerHTML = "<g></g><g></g>";
  for (const form of FORM_KEYS) {
    const engine = new Companion({ form, auto: false });
    for (const action of [() => engine.poke(), () => engine.sleep(), () => engine.wake(), () => engine.startWalk(), () => engine.selectForm("ghost"), () => engine.selectForm("mochi")]) {
      action();
      for (let i = 0; i < 80; i++) {
        engine.update(1 / 60);
        paintMilkBody(body, feet, engine);
        expect(body.getAttribute("d")).not.toMatch(/NaN|Infinity/);
        for (const foot of feet.children) expect(foot.getAttribute("transform")).not.toMatch(/NaN|Infinity/);
      }
    }
  }
});
