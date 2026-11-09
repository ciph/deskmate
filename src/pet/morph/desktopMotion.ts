import {
  currentMonitor,
  primaryMonitor,
  getCurrentWindow,
  PhysicalPosition,
} from "@tauri-apps/api/window";
import type { Companion, Point } from "./engine";

export const morphEnvelope = { horizontal: 1, top: 1.2, bottom: 0.2 };
/**
 * Seconds without a native move sample after which the dragging hand counts as still.
 * Longer than the 40 ms release poll plus IPC so a real throw keeps its speed.
 */
const HAND_STILL = 0.12;
export interface DesktopGeometry {
  position: Point;
  size: { width: number; height: number };
  ratio: number;
  area: { x: number; y: number; width: number; height: number };
}
export interface MotionHost {
  read(): Promise<DesktopGeometry>;
  write(position: Point): Promise<void>;
}
export function nativeMotionHost(): MotionHost {
  const pet = getCurrentWindow();
  return {
    async read() {
      const [position, size, ratio, display] = await Promise.all([
        pet.outerPosition(),
        pet.outerSize(),
        pet.scaleFactor(),
        currentMonitor(),
      ]);
      const monitor = display ?? (await primaryMonitor());
      if (!monitor || !Number.isFinite(ratio) || ratio <= 0)
        throw new Error("Desktop geometry unavailable");
      return {
        position,
        size,
        ratio,
        area: {
          x: monitor.workArea.position.x,
          y: monitor.workArea.position.y,
          width: monitor.workArea.size.width,
          height: monitor.workArea.size.height,
        },
      };
    },
    write: (point) =>
      pet.setPosition(
        new PhysicalPosition(Math.round(point.x), Math.round(point.y)),
      ),
  };
}
export function desktopAnchor(g: DesktopGeometry, size: number): Point {
  return {
    x: (g.position.x - g.area.x + g.size.width / 2) / g.ratio,
    y:
      (g.position.y - g.area.y + g.size.height) / g.ratio -
      size * morphEnvelope.bottom,
  };
}
export function desktopOrigin(
  g: DesktopGeometry,
  size: number,
  point: Point,
): Point {
  return {
    x: g.area.x + point.x * g.ratio - g.size.width / 2,
    y:
      g.area.y +
      (point.y + size * morphEnvelope.bottom) * g.ratio -
      g.size.height,
  };
}

/** Only actual locomotion moves the native window. Idle breathing stays in SVG. */
export function createDesktopMotion(
  engine: Companion,
  host: MotionHost,
  fail: (error: unknown) => void,
) {
  let geometry: DesktopGeometry | null = null,
    active = false,
    disposed = false,
    dragging = false;
  let version = 0,
    writing = false,
    syncing = false,
    written: Point | null = null,
    sampledAt = 0;
  const sample = (position: Point, trackVelocity = false) => {
    if (!geometry) return;
    const anchor = desktopAnchor({ ...geometry, position }, engine.size),
      previous = desktopAnchor(geometry, engine.size);
    geometry = { ...geometry, position };
    engine.x = anchor.x;
    engine.y = anchor.y;
    if (engine.grab) {
      const offset = engine.deform(engine.grab.anchor.x, engine.grab.anchor.y);
      engine.grab.point = {
        x: anchor.x + offset[0] * engine.scale,
        y: anchor.y + offset[1] * engine.scale,
      };
      // A release/resize read is not a motion sample: keep the hand's last velocity.
      if (trackVelocity) {
        const dt = Math.max(0.008, Math.min(0.1, engine.clock - sampledAt));
        sampledAt = engine.clock;
        engine.grab.vx = (anchor.x - previous.x) / dt;
        engine.grab.vy = (anchor.y - previous.y) / dt;
      }
    }
  };
  const sync = async () => {
    const generation = ++version;
    syncing = true;
    try {
      const next = await host.read();
      if (disposed || generation !== version) return;
      geometry = next;
      written = next.position;
      engine.setBounds(
        next.area.width / next.ratio,
        next.area.height / next.ratio + 58 - engine.size * morphEnvelope.bottom,
      );
      sample(next.position);
      const limits = engine.limits;
      engine.x = Math.max(limits.left, Math.min(limits.right, engine.x));
      engine.y = Math.max(limits.top, Math.min(engine.floor, engine.y));
    } finally {
      if (generation === version) syncing = false;
    }
  };
  return {
    sync,
    get groundOffset(): number | null {
      if (!geometry) return null;
      // Use the observed native position: the OS owns movement while dragging.
      return (geometry.area.y + geometry.area.height - geometry.position.y - geometry.size.height) /
        geometry.ratio / engine.scale;
    },
    setActive(value: boolean) {
      active = value;
      if (!value) {
        engine.stopWalk();
        engine.release(true);
        dragging = false;
      }
    },
    moved(position: Point) {
      if (disposed || !geometry) return;
      if (dragging) {
        sample(position, true);
        return;
      }
      geometry = { ...geometry, position };
      if (
        written &&
        Math.hypot(position.x - written.x, position.y - written.y) < 3
      )
        return;
      if (writing || syncing) return;
      void sync().catch(fail);
    },
    drag(value: boolean) {
      if (disposed) return;
      if (value) {
        if (!active || !geometry) return;
        dragging = true;
        sampledAt = engine.clock;
        engine.stopWalk();
        engine.beginDrag(
          { x: engine.x, y: engine.y - 100 * engine.scale },
          { x: 0, y: -100 },
        );
      } else if (dragging) {
        dragging = false;
        void sync()
          .then(() => {
            if (!disposed) engine.release(!active);
          })
          .catch((error) => {
            engine.release(true);
            fail(error);
          });
      }
    },
    tick(dt: number) {
      if (!active || disposed || syncing || !geometry) return;
      engine.update(dt);
      // The OS reports drag motion only while the hand moves; a quiet window means it stopped.
      if (dragging && engine.grab && engine.clock - sampledAt > HAND_STILL) {
        engine.grab.vx = 0;
        engine.grab.vy = 0;
      }
      if (dragging || writing) return;
      const position = desktopOrigin(geometry, engine.size, engine);
      if (
        written &&
        Math.abs(position.x - written.x) < 0.75 &&
        Math.abs(position.y - written.y) < 0.75
      )
        return;
      written = position;
      writing = true;
      void host
        .write(position)
        .catch((error) => {
          active = false;
          fail(error);
        })
        .finally(() => {
          writing = false;
        });
    },
    dispose() {
      disposed = true;
      active = false;
      version++;
      engine.release(true);
    },
  };
}
