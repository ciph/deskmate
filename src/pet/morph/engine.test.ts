import assert from "node:assert/strict";
import { test } from "bun:test";
import { Companion, FORM_KEYS, type FormKey } from "./engine";
import model from "./model.json";

const tick = (pet: Companion, seconds: number) => {
  for (let i = 0; i < Math.ceil(seconds * 60); i++) pet.update(1 / 60);
};
const make = (form: FormKey) =>
  new Companion({ width: 840, height: 506, form, auto: false });
const check = test;
for (const form of FORM_KEYS) {
  check(
    form +
      ": visible squash, stretch and pickup expression recover after release",
    () => {
      const p = make(form);
      const rest = p.frame().mouth;
      p.poke();
      let squeeze = 0,
        dent = 0;
      for (let i = 0; i < 90; i++) {
        p.update(1 / 60);
        squeeze = Math.max(squeeze, Math.abs(p.p.sy - 1), Math.abs(p.p.sx - 1));
        dent = Math.max(dent, p.p.dent);
      }
      assert.ok(squeeze > 0.08, `${form}: squeeze ${squeeze}`);
      assert.ok(dent > 10, `${form}: dent ${dent}`);
      p.beginDrag({ x: p.x, y: p.y - 100 * p.scale });
      tick(p, 0.4);
      assert.ok(p.p.sy > 1.16);
      assert.ok(p.p.sx < 0.94);
      assert.ok(p.frame().expression > 0.95);
      assert.notEqual(p.frame().mouth, rest);
      p.release();
      tick(p, 6);
      assert.equal(p.state, "idle");
      assert.ok(p.frame().expression < 0.001);
    },
  );
  check(form + ": poke, sleep, wake", () => {
    const p = make(form);
    p.poke();
    tick(p, 2.5);
    assert.equal(p.state, "idle");
    p.sleep();
    tick(p, 3.3);
    assert.equal(p.state, "asleep");
    p.poke();
    assert.equal(p.state, "waking");
    tick(p, 2.5);
    assert.equal(p.state, "idle");
    assert.ok(p.p.rest < 0.001);
  });
  check(form + ": walking, grip, release", () => {
    const p = make(form);
    p.startWalk({ direction: 1 });
    tick(p, 2);
    assert.ok(p.x > 420);
    const start = { x: p.x, y: p.y - 100 * p.scale, time: 0 };
    p.beginDrag(start);
    p.moveDrag({ x: 420, y: 240, time: 100 });
    tick(p, 0.4);
    assert.equal(p.state, "dragging");
    assert.ok(p.grab);
    const q = p.deform(p.grab.anchor.x, p.grab.anchor.y);
    assert.ok(Math.abs(p.x + q[0] * p.scale - 420) < 0.01);
    assert.ok(Math.abs(p.y + q[1] * p.scale - 240) < 0.01);
    p.release();
    tick(p, 6);
    assert.equal(p.state, "idle");
    assert.equal(p.y, p.floor);
  });
  check(form + ": queued shape and extreme boundaries", () => {
    const p = make(form);
    p.beginDrag({ x: p.x, y: p.y - 100 * p.scale, time: 0 });
    const target = form === "mochi" ? "ghost" : "mochi";
    p.selectForm(target);
    assert.equal(p.form, form);
    assert.equal(p.pendingForm, target);
    p.moveDrag({ x: 10000, y: -10000, time: 16 });
    tick(p, 0.2);
    p.release();
    assert.equal(p.form, target);
    tick(p, 8);
    assert.equal(p.state, "idle");
    assert.equal(p.morph, null);
    assert.equal(p.pendingForm, null);
    assert.ok(p.x >= p.limits.left && p.x <= p.limits.right);
    assert.equal(p.y, p.floor);
    for (const size of [64, 96, 128, 220]) {
      p.setSize(size);
      p.setBounds(300, 400);
      tick(p, 0.3);
      assert.ok(p.x >= p.limits.left && p.x <= p.limits.right);
      assert.ok(!/NaN|Infinity/.test(p.frame().d));
    }
  });
}
check("rapid shape changes preserve the current outline", () => {
  const p = make("mochi");
  for (let i = 0; i < 36; i++) {
    const before = [...p.model.outline];
    p.selectForm(FORM_KEYS[i % 4]);
    assert.deepEqual(p.model.outline, before);
    tick(p, 0.1);
    assert.equal(p.model.outline.length, 98);
    assert.ok(!/NaN|Infinity/.test(p.frame().d));
  }
  tick(p, 1);
  assert.equal(p.morph, null);
  assert.ok(
    p.model.outline.every(
      (v, i) => Math.abs(v - model.forms.ghost.outline[i]) < 1e-9,
    ),
  );
});
check("ghost lands while morphing back into mochi", () => {
  const p = make("ghost");
  tick(p, 1);
  const hover = p.p.lift;
  p.selectForm("mochi");
  assert.equal(p.p.lift, hover);
  tick(p, 0.4);
  assert.ok(p.morph, "shape transition must still be in progress");
  assert.ok(p.model.ghost > 0 && p.model.ghost < 1);
  assert.ok(p.p.lift > hover + 6, "descent must start before morphing ends");
  assert.ok(p.p.lift < -5, "descent must stay gradual");
  tick(p, 0.4);
  assert.equal(p.morph, null);
  assert.ok(Math.abs(p.p.lift) < 2, "reach the floor with the new shape");
  tick(p, 0.5);
  assert.ok(Math.abs(p.p.lift) < 0.02);
  assert.equal(p.state, "idle");
});
check("cancelled drag releases safely", () => {
  const p = make("pillow");
  p.beginDrag({ x: 420, y: 360, time: 0 });
  p.moveDrag({ x: 300, y: 220, time: 80 });
  tick(p, 0.3);
  p.release(true);
  assert.equal(p.vx, 0);
  tick(p, 3);
  assert.equal(p.state, "idle");
  assert.equal(p.grab, null);
});
check("autonomous walking and sleep; disabled autonomy stays idle", () => {
  const p = make("mochi");
  p.auto = true;
  tick(p, 23);
  assert.equal(p.state, "walking");
  tick(p, 42);
  assert.equal(p.state, "asleep");
  p.poke();
  tick(p, 2);
  assert.equal(p.state, "idle");
  assert.ok(p.quiet < 3);
  const q = make("ghost");
  tick(q, 90);
  assert.equal(q.state, "idle");
});
