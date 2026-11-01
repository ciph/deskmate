import { expect, test } from "bun:test";
import { Companion } from "./engine";
import {
  createDesktopMotion,
  desktopAnchor,
  desktopOrigin,
  type DesktopGeometry,
} from "./desktopMotion";
const geometry: DesktopGeometry = {
  position: { x: -1200, y: 400 },
  size: { width: 640, height: 484 },
  ratio: 2,
  area: { x: -1920, y: 100, width: 1920, height: 1100 },
};
const flush = async () => {
  await Promise.resolve();
  await Promise.resolve();
};
test("desktop coordinates round-trip on a negative-origin screen at 200% DPI", () => {
  expect(desktopOrigin(geometry, 160, desktopAnchor(geometry, 160))).toEqual(
    geometry.position,
  );
});
test("native walking moves the window; hiding and disposal stop all writes", async () => {
  const engine = new Companion({ size: 160, auto: false });
  const writes: { x: number; y: number }[] = [];
  const motion = createDesktopMotion(
    engine,
    {
      read: async () => geometry,
      write: async (p) => {
        writes.push(p);
      },
    },
    (error) => {
      throw error;
    },
  );
  await motion.sync();
  motion.setActive(true);
  engine.y = engine.floor;
  engine.startWalk({ direction: 1 });
  for (let n = 0; n < 80; n++) {
    motion.tick(1 / 30);
    await flush();
  }
  expect(writes.length).toBeGreaterThan(10);
  expect(writes.at(-1)!.x).toBeGreaterThan(writes[0].x);
  motion.setActive(false);
  const count = writes.length;
  for (let n = 0; n < 80; n++) motion.tick(1 / 30);
  expect(writes).toHaveLength(count);
  motion.dispose();
  motion.setActive(true);
  motion.tick(0.05);
  expect(writes).toHaveLength(count);
});
test("the OS owns drag movement, then release falls within the work area", async () => {
  const engine = new Companion({ size: 160, auto: false });
  let native = { ...geometry };
  const writes: { x: number; y: number }[] = [];
  const motion = createDesktopMotion(
    engine,
    {
      read: async () => native,
      write: async (p) => {
        writes.push(p);
        native = { ...native, position: p };
      },
    },
    (error) => {
      throw error;
    },
  );
  await motion.sync();
  motion.setActive(true);
  motion.drag(true);
  native = { ...native, position: { x: -900, y: 260 } };
  motion.moved(native.position);
  motion.tick(0.03);
  const velocity = engine.grab!.vx;
  expect(velocity).toBeGreaterThan(0);
  expect(writes).toHaveLength(0);
  expect(engine.state).toBe("dragging");
  engine.selectForm("ghost");
  motion.drag(false);
  await flush();
  await flush();
  expect(engine.vx).toBeGreaterThan(0);
  for (let n = 0; n < 250; n++) {
    motion.tick(1 / 30);
    await flush();
  }
  expect(engine.form).toBe("ghost");
  expect(engine.state).toBe("idle");
  expect(engine.y).toBe(engine.floor);
  const last = writes.at(-1)!;
  expect(last.x).toBeGreaterThanOrEqual(geometry.area.x - 1);
  expect(last.y + geometry.size.height).toBeLessThanOrEqual(
    geometry.area.y + geometry.area.height + 1,
  );
});
test("a hand that stops before letting go drops straight down instead of being thrown", async () => {
  const engine = new Companion({ size: 160, auto: false });
  let native = { ...geometry };
  const motion = createDesktopMotion(
    engine,
    {
      read: async () => native,
      write: async (p) => {
        native = { ...native, position: p };
      },
    },
    (error) => {
      throw error;
    },
  );
  await motion.sync();
  motion.setActive(true);
  motion.drag(true);
  for (let n = 1; n <= 10; n++) {
    native = { ...native, position: { x: -1200 - 20 * n, y: 400 - 18 * n } };
    motion.tick(1 / 60);
    motion.moved(native.position);
  }
  expect(engine.grab!.vx).toBeLessThan(-100);
  // Hand rests for 0.3 s: the OS sends no further move events.
  for (let n = 0; n < 18; n++) motion.tick(1 / 60);
  expect(engine.grab!.vx).toBe(0);
  expect(engine.grab!.vy).toBe(0);
  motion.drag(false);
  await flush();
  await flush();
  expect(engine.state).toBe("falling");
  expect(engine.vx).toBe(0);
  const x = engine.x;
  for (let n = 0; n < 120; n++) {
    motion.tick(1 / 60);
    await flush();
  }
  expect(Math.abs(engine.x - x)).toBeLessThan(0.5);
});
test("a stale geometry read cannot move a disposed pet", async () => {
  let resolve!: (value: DesktopGeometry) => void;
  const waiting = new Promise<DesktopGeometry>((done) => {
    resolve = done;
  });
  let writes = 0;
  const motion = createDesktopMotion(
    new Companion(),
    {
      read: () => waiting,
      write: async () => {
        writes++;
      },
    },
    (error) => {
      throw error;
    },
  );
  const ready = motion.sync();
  motion.setActive(true);
  motion.dispose();
  resolve(geometry);
  await ready;
  motion.tick(0.05);
  expect(writes).toBe(0);
});
