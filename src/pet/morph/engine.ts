import data from "./model.json";
export const FORM_KEYS = ["mochi", "pillow", "ricecake", "ghost"] as const;
export type FormKey = (typeof FORM_KEYS)[number];
export const isFormKey = (value: unknown): value is FormKey =>
  typeof value === "string" && (FORM_KEYS as readonly string[]).includes(value);
export interface Point {
  x: number;
  y: number;
  time?: number;
}
export type MotionState =
  | "idle"
  | "poke"
  | "walking"
  | "drowsy"
  | "asleep"
  | "waking"
  | "dragging"
  | "falling"
  | "landing";
export interface Shape {
  name: string;
  faceY: number;
  ghost: number;
  outline: number[];
}
const M: { forms: Record<FormKey, Shape> } = data;
// The approved playground's four outlines share one topology and one rigid face.
const clamp = (v: number, a = 0, b = 1) => Math.max(a, Math.min(b, v));
const smooth = (v: number) => {
  v = clamp(v);
  return v * v * v * (v * (6 * v - 15) + 10);
};
const bump = (t: number, a: number, b: number, c: number) =>
  t < a || t > c
    ? 0
    : t <= b
      ? smooth((t - a) / (b - a))
      : 1 - smooth((t - b) / (c - b));
const ripple = (t: number, a: number, len: number) => {
  const u = (t - a) / len;
  return u < 0 || u > 1
    ? 0
    : Math.sin(4 * Math.PI * u) * Math.sin(Math.PI * u) * Math.exp(-2 * u);
};
const mix = (a: number, b: number, t: number) => a + (b - a) * t,
  TAU = Math.PI * 2;
const round = (v: number) => Number(v.toFixed(3));
const neutral = () => ({
  sx: 1,
  sy: 1,
  lift: 0,
  lean: 0,
  lag: 0,
  dent: 0,
  rest: 0,
  yawn: 0,
  surprise: 0,
  deep: 0,
  faceExtra: 0,
  walk: 0,
  hang: 0,
  wave: 0,
});
const descriptions = {
  idle: "安静地陪着你",
  poke: "嘿嘿，有点痒",
  walking: "出去溜达一下",
  drowsy: "有一点点困了",
  asleep: "呼……睡着啦",
  waking: "唔，我醒啦",
  dragging: "被你拎起来啦",
  falling: "软软地落下来",
  landing: "站稳，站稳",
};

export class Companion {
  width: number;
  height: number;
  size: number;
  form: FormKey;
  model: Shape;
  morph: { from: Shape; to: Shape; elapsed: number } | null = null;
  pendingForm: FormKey | null = null;
  clock = 0;
  age = 0;
  quiet = 0;
  nextWander = 22;
  auto = true;
  state: MotionState = "idle";
  x: number;
  y: number;
  vx = 0;
  vy = 0;
  p = neutral();
  hitSide = 1;
  hitY = -100;
  walkPhase = 0;
  walkDistance = 0;
  walkDir = 1;
  walkUntil = 0;
  walkPause = 0;
  walkFrom = 0;
  walkTo = 0;
  wakeRest = 0;
  grab: {
    anchor: Point;
    point: Point;
    vx: number;
    vy: number;
    lastTime: number;
  } | null = null;
  constructor({
    width = 800,
    height = 480,
    size = 220,
    form = "mochi",
    auto = true,
  }: {
    width?: number;
    height?: number;
    size?: number;
    form?: FormKey;
    auto?: boolean;
  } = {}) {
    this.width = width;
    this.height = height;
    this.size = size;
    this.form = M.forms[form] ? form : "mochi";
    this.model = {
      ...M.forms[this.form],
      outline: [...M.forms[this.form].outline],
    };
    this.morph = null;
    this.pendingForm = null;
    this.clock = 0;
    this.age = 0;
    this.quiet = 0;
    this.nextWander = 22;
    this.auto = auto;
    this.state = "idle";
    this.x = width / 2;
    this.y = this.floor;
    this.vx = 0;
    this.vy = 0;
    this.p = neutral();
    this.p.lift = this.form === "ghost" ? -28 : 0;
    this.hitSide = 1;
    this.hitY = -100;
    this.walkPhase = 0;
    this.walkDistance = 0;
    this.walkDir = 1;
    this.grab = null;
  }
  get scale() {
    return this.size / 270;
  }
  get floor() {
    return Math.max(100, this.height - 58);
  }
  get limits() {
    const margin = Math.min(this.width / 2, 180 * this.scale + 10);
    return {
      left: margin,
      right: this.width - margin,
      top: Math.min(this.floor, 255 * this.scale + 12),
    };
  }
  get busy() {
    return this.state === "dragging" || this.state === "falling";
  }
  get status() {
    if (this.pendingForm) return `松手后变成${M.forms[this.pendingForm].name}`;
    if (this.morph) return `正在变成${M.forms[this.form].name}`;
    return descriptions[this.state] || descriptions.idle;
  }
  input() {
    this.quiet = 0;
    this.nextWander = this.clock + 24;
  }
  change(state: MotionState) {
    this.state = state;
    this.age = 0;
  }
  setBounds(width: number, height: number) {
    const grounded = [
      "idle",
      "poke",
      "walking",
      "drowsy",
      "asleep",
      "waking",
      "landing",
    ].includes(this.state);
    this.width = Math.max(240, width);
    this.height = Math.max(280, height);
    if (grounded) this.y = this.floor;
    const l = this.limits;
    this.x = clamp(this.x, l.left, l.right);
    this.y = clamp(this.y, l.top, this.floor);
    if (this.state === "walking") this.planStep();
  }
  setSize(size: number) {
    if (!Number.isFinite(size) || size < 32 || size > 640) return;
    if (this.grab) this.release();
    this.size = size;
    this.setBounds(this.width, this.height);
    this.input();
  }
  selectForm(key: FormKey) {
    if (!M.forms[key]) return false;
    this.input();
    if (this.state === "dragging") {
      this.pendingForm = key;
      return true;
    }
    if (key === this.form && !this.morph) return true;
    if (this.state === "walking") this.change("idle");
    if (["asleep", "drowsy"].includes(this.state)) this.wake();
    this.morph = {
      from: { ...this.model, outline: [...this.model.outline] },
      to: M.forms[key],
      elapsed: 0,
    };
    this.form = key;
    return true;
  }
  sleep(automatic = false) {
    if (this.busy) return false;
    if (!automatic) this.input();
    this.vx = 0;
    this.change("drowsy");
    return true;
  }
  wake() {
    this.input();
    this.wakeRest = this.p.rest;
    this.change("waking");
    return true;
  }
  poke(point?: Point) {
    this.input();
    if (this.state === "dragging") return false;
    if (["asleep", "drowsy"].includes(this.state)) return this.wake();
    this.hitSide = point ? Math.sign(point.x - this.x) || 1 : 1;
    this.hitY = point
      ? clamp((point.y - this.y) / this.scale, -200, -30)
      : -100;
    this.change("poke");
    return true;
  }
  startWalk({
    direction,
    duration = Infinity,
    automatic = false,
  }: { direction?: number; duration?: number; automatic?: boolean } = {}) {
    if (this.busy) return false;
    if (!automatic) this.input();
    this.walkDir = direction || (this.x < this.width / 2 ? 1 : -1);
    this.walkUntil = this.clock + duration;
    this.walkPhase = 0;
    this.walkDistance = 0;
    this.walkPause = 0;
    this.planStep();
    this.change("walking");
    return true;
  }
  stopWalk() {
    if (this.state === "walking") {
      this.change("idle");
      this.input();
    }
  }
  planStep() {
    const l = this.limits;
    if (this.x >= l.right - 2) this.walkDir = -1;
    if (this.x <= l.left + 2) this.walkDir = 1;
    this.walkFrom = this.x;
    this.walkTo = clamp(
      this.x + this.walkDir * (this.form === "pillow" ? 40 : 47) * this.scale,
      l.left,
      l.right,
    );
  }
  home() {
    this.grab = null;
    this.pendingForm = null;
    this.x = this.width / 2;
    this.y = this.floor;
    this.vx = 0;
    this.vy = 0;
    this.change("idle");
    this.p = neutral();
    this.p.lift = this.form === "ghost" ? -28 : 0;
    this.input();
  }
  anchorAt(point: Point) {
    const local = {
      x: (point.x - this.x) / this.scale,
      y: (point.y - this.y) / this.scale,
    };
    let a = { x: local.x, y: local.y };
    for (let i = 0; i < 6; i++) {
      const q = this.deform(a.x, a.y);
      a.x += (local.x - q[0]) / this.p.sx;
      a.y += (local.y - q[1]) / this.p.sy;
    }
    return a;
  }
  beginDrag(point: Point, anchor?: Point) {
    this.input();
    this.grab = {
      anchor: anchor || this.anchorAt(point),
      point: { ...point },
      vx: 0,
      vy: 0,
      lastTime: point.time || 0,
    };
    this.vx = 0;
    this.vy = 0;
    this.change("dragging");
  }
  moveDrag(point: Point) {
    if (!this.grab) return;
    const g = this.grab,
      dt = clamp(((point.time || 0) - g.lastTime) / 1000, 0.008, 0.1);
    g.vx = mix(g.vx, (point.x - g.point.x) / dt, 0.35);
    g.vy = mix(g.vy, (point.y - g.point.y) / dt, 0.35);
    g.point = { ...point };
    g.lastTime = point.time || 0;
  }
  release(cancelled = false) {
    if (!this.grab) return;
    this.vx = cancelled ? 0 : clamp(this.grab.vx, -600, 600);
    this.vy = cancelled ? 0 : clamp(this.grab.vy, -500, 700);
    this.grab = null;
    this.change("falling");
    this.input();
    if (this.pendingForm) {
      const key = this.pendingForm;
      this.pendingForm = null;
      this.selectForm(key);
    }
  }
  target() {
    const q = neutral(),
      t = this.age,
      key = this.form,
      ghost = this.model.ghost;
    q.lift = -28 * ghost;
    q.sy = 1 + 0.006 * (1 - Math.cos(this.clock * 1.7));
    if (this.state === "poke") {
      const press = bump(t, 0, 0.15, 0.36),
        hop = bump(t, 0.3, 0.62, 1.04);
      q.surprise = 0.75 * press;
      if (key === "mochi") {
        q.sy = 1 - 0.24 * press + 0.13 * hop - 0.055 * ripple(t, 1.05, 0.8);
        q.sx = 1 + 0.18 * press - 0.07 * hop;
        q.lift = -30 * hop;
        q.dent = 28 * press;
      }
      if (key === "pillow") {
        const sag = bump(t, 0, 0.25, 1.8);
        q.sy = 1 - 0.24 * sag;
        q.sx = 1 + 0.2 * sag;
        q.dent = 30 * sag;
        q.lean = -this.hitSide * 8 * sag;
      }
      if (key === "ricecake") {
        q.sy = 1 - 0.12 * press + 0.1 * hop;
        q.sx = 1 + 0.08 * press - 0.04 * hop;
        q.lift = -14 * hop;
        q.lean = -this.hitSide * 12 * press + 20 * ripple(t, 0.45, 1.25);
        q.dent = 16 * press;
      }
      if (key === "ghost") {
        const dodge = bump(t, 0.08, 0.65, 1.95);
        q.sy = 1 - 0.14 * press + 0.1 * dodge;
        q.sx = 1 + 0.1 * press - 0.04 * dodge;
        q.lean = -this.hitSide * 28 * dodge;
        q.lift = -28 - 18 * dodge;
        q.dent = 20 * press;
        q.wave = 7 * dodge;
      }
    } else if (this.state === "dragging") {
      q.hang = 1;
      q.surprise = 1;
      q.sy =
        1 +
        { mochi: 0.34, pillow: 0.3, ricecake: 0.19, ghost: 0.24 }[key] +
        0.05 * clamp(Math.abs(this.grab?.vy || 0) / 600) +
        0.018 * Math.sin(t * 7) * Math.exp(-t * 1.6);
      q.sx = 1 / Math.sqrt(q.sy);
      q.lag = -clamp((this.grab?.vx || 0) / 18, -32, 32);
      q.lean = clamp((this.grab?.vx || 0) / 45, -10, 10);
      q.lift = 4;
    } else if (this.state === "falling") {
      q.surprise = 0.65;
      q.sy = 1.08;
      q.sx = 0.96;
      q.lag = -clamp(this.vx / 45, -10, 10);
      q.lift = -28 * ghost;
    } else if (this.state === "landing") {
      const depth = { mochi: 0.25, pillow: 0.26, ricecake: 0.16, ghost: 0.075 }[
        key
      ];
      const flat = bump(t, 0, 0.18, key === "pillow" ? 1.1 : 0.5),
        hop = key === "mochi" ? bump(t, 0.3, 0.55, 0.9) : 0;
      q.sy = 1 - depth * flat + 0.04 * hop;
      q.sx = 1 + depth * 0.8 * flat;
      q.lift = -28 * ghost - 10 * hop;
    } else if (["drowsy", "asleep", "waking"].includes(this.state)) {
      const isRice = key === "ricecake",
        isPillow = key === "pillow";
      let rest = 1,
        yawn = 0,
        stretch = 0,
        nod = 0;
      if (this.state === "drowsy") {
        rest = smooth((t - (isRice ? 1.4 : 0.7)) / (isRice ? 1.5 : 1.6));
        nod = isRice
          ? 0.7 * bump(t, 0.15, 0.6, 1.1) + bump(t, 1.15, 1.6, 2.1)
          : bump(t, 0.1, 0.65, 1.3);
        yawn = bump(t, 0.15, 0.75, 1.5) * (isRice ? 0.45 : 1);
      } else if (this.state === "waking") {
        const duration = isPillow ? 1.5 : 1.0;
        rest = (this.wakeRest ?? 1) * (1 - smooth(t / duration));
        stretch = bump(t, duration * 0.5, duration, duration + 0.5);
        yawn =
          bump(t, 0.2, duration * 0.8, duration + 0.4) * (isRice ? 0.55 : 1);
      }
      q.rest = rest;
      q.yawn = yawn;
      q.deep = this.state === "asleep" ? 1 : 0;
      const depth = { mochi: 0.32, pillow: 0.38, ricecake: 0.18, ghost: 0.15 }[
        key
      ];
      q.sy =
        1 -
        depth * rest -
        0.04 * nod +
        0.06 * stretch +
        0.008 * Math.sin(this.clock * 2.5) * rest;
      q.sx =
        1 +
        { mochi: 0.18, pillow: 0.16, ricecake: 0.055, ghost: 0.09 }[key] *
          rest -
        0.035 * stretch;
      q.lean = (isRice ? 9 : 3) * rest + 3 * nod;
      q.faceExtra = 2 * rest;
      q.lift = (-28 + 20 * rest - 3 * stretch) * ghost;
    } else if (this.state === "walking") {
      const p = this.walkPhase;
      q.walk = this.walkPause > 0 ? 0 : 1;
      if (key === "mochi") {
        const fly = bump(p, 0.18, 0.48, 0.78);
        q.sy =
          1 -
          0.055 * bump(p, 0, 0.11, 0.22) +
          0.04 * fly -
          0.07 * bump(p, 0.76, 0.87, 1);
        q.lift = -15 * fly;
        q.lean = this.walkDir * 4 * fly;
      } else if (key === "pillow") {
        const pulse = Math.sin(Math.PI * p) ** 2;
        q.sy = 1 - 0.06 * pulse;
        q.sx = 1 + 0.05 * pulse;
        q.lean = this.walkDir * 3 * pulse;
      } else if (key === "ricecake") {
        q.lift = -2.5 * Math.sin(TAU * p) ** 2;
        q.sy = 1 + 0.01 * Math.cos(2 * TAU * p);
        q.lean = this.walkDir * 3;
      } else {
        q.lift = -28 - 5 * Math.sin(this.clock * 2);
        q.lag = -this.walkDir * 6;
        q.wave = 2.5;
      }
    } else if (ghost) {
      q.lift = -28 - 3 * Math.sin(this.clock * 1.8);
    }
    return q;
  }
  update(dt: number) {
    dt = clamp(dt, 0, 0.05);
    this.clock += dt;
    this.age += dt;
    this.quiet += dt;
    if (this.morph) {
      const m = this.morph;
      m.elapsed += dt;
      const k = smooth(m.elapsed / 0.8);
      this.model = {
        ...m.to,
        outline: m.from.outline.map((v, i) => mix(v, m.to.outline[i], k)),
        faceY: mix(m.from.faceY, m.to.faceY, k),
        ghost: mix(m.from.ghost, m.to.ghost, k),
      };
      if (k === 1) this.morph = null;
    }
    if (this.state === "walking") {
      if (this.clock >= this.walkUntil) {
        this.change("idle");
      } else if (this.walkPause > 0) {
        this.walkPause -= dt;
      } else if (["mochi", "pillow"].includes(this.form)) {
        this.walkPhase += dt / (this.form === "pillow" ? 1.2 : 0.9);
        const from = this.form === "pillow" ? 0.14 : 0.18,
          to = this.form === "pillow" ? 0.69 : 0.6;
        this.x = mix(
          this.walkFrom,
          this.walkTo,
          smooth((this.walkPhase - from) / to),
        );
        if (this.walkPhase >= 1) {
          this.x = this.walkTo;
          this.walkPhase = 0;
          const l = this.limits;
          if (this.x <= l.left + 1 || this.x >= l.right - 1)
            this.walkPause = 0.5;
          this.planStep();
        }
      } else {
        const distance = (this.form === "ghost" ? 70 : 58) * this.scale * dt;
        this.x += this.walkDir * distance;
        this.walkDistance += distance;
        this.walkPhase = (this.walkDistance / (40 * this.scale)) % 1;
        const l = this.limits;
        if (this.x <= l.left || this.x >= l.right) {
          this.x = clamp(this.x, l.left, l.right);
          this.walkDir *= -1;
          this.walkPause = 0.55;
        }
      }
    }
    const target = this.target(),
      blend =
        1 -
        Math.exp(
          -dt /
            (this.state === "poke"
              ? 0.045
              : this.state === "dragging"
                ? 0.065
                : 0.1),
        );
    for (const k of Object.keys(this.p) as (keyof typeof this.p)[])
      this.p[k] = mix(this.p[k], target[k], blend);
    if (this.state === "dragging" && this.grab) {
      const a = this.deform(this.grab.anchor.x, this.grab.anchor.y),
        l = this.limits;
      this.x = clamp(this.grab.point.x - a[0] * this.scale, l.left, l.right);
      this.y = clamp(this.grab.point.y - a[1] * this.scale, l.top, this.floor);
      this.grab.vx *= Math.exp(-dt * 2);
      this.grab.vy *= Math.exp(-dt * 2);
    } else if (this.state === "falling" || this.y < this.floor - 0.2) {
      if (this.form === "ghost")
        this.vy += ((this.floor - this.y) * 15 - this.vy * 7) * dt;
      else this.vy += 1600 * dt;
      this.x += this.vx * dt;
      this.y += this.vy * dt;
      this.vx *= Math.exp(-dt * 3);
      const l = this.limits;
      if (this.x < l.left || this.x > l.right) {
        this.x = clamp(this.x, l.left, l.right);
        this.vx *= -0.22;
      }
      if (this.y < l.top) {
        this.y = l.top;
        this.vy = Math.max(0, this.vy) * 0.2;
      }
      if (
        this.y >= this.floor ||
        (this.form === "ghost" &&
          Math.abs(this.floor - this.y) < 0.7 &&
          Math.abs(this.vy) < 5)
      ) {
        this.y = this.floor;
        this.vx = 0;
        this.vy = 0;
        this.change(this.form === "ghost" ? "idle" : "landing");
      }
    }
    if (this.state === "poke" && this.age > 2.3) this.change("idle");
    if (this.state === "landing" && this.age > 1.25) this.change("idle");
    if (this.state === "drowsy" && this.age > 3.1) this.change("asleep");
    if (
      this.state === "waking" &&
      this.age > (this.form === "pillow" ? 2.1 : 1.7)
    )
      this.change("idle");
    if (this.auto && this.state === "idle" && !this.morph) {
      if (this.quiet > 60) this.sleep(true);
      else if (this.clock > this.nextWander && this.quiet > 8) {
        this.nextWander = this.clock + 27;
        this.startWalk({ automatic: true, duration: 4.3 });
      }
    }
  }
  deform(x: number, y: number): [number, number] {
    const p = this.p,
      upper = clamp(-y / 200),
      lower = clamp((y - this.model.outline[1]) / -this.model.outline[1]);
    let px = x * p.sx + p.lean * upper + p.lag * lower ** 1.3,
      py = y * p.sy + p.lift;
    px -=
      this.hitSide *
      p.dent *
      Math.exp(-(((y - this.hitY) / 34) ** 2)) *
      clamp((x * this.hitSide - 20) / 60);
    if (this.form === "pillow")
      py +=
        (2.5 * p.rest + 18 * p.hang) *
        (Math.abs(x) / 125) ** 2 *
        (0.8 + 0.2 * upper);
    if (this.form === "ricecake") {
      const arm =
        Math.exp(-(((y + 94) / 22) ** 2)) * clamp((Math.abs(x) - 40) / 50);
      px -= Math.sign(x) * 2.5 * p.rest * arm;
      if (p.walk > 0.001) {
        const phase = (this.walkPhase + (x < 0 ? 0.5 : 0)) % 1,
          weight =
            clamp((y + 23) / 25) * Math.exp(-(((Math.abs(x) - 53) / 25) ** 2));
        const u = phase < 0.5 ? 0 : (phase - 0.5) * 2;
        const fx =
            phase < 0.5
              ? 10 - 40 * phase
              : 10 * (-1 - 2 * u + 12 * u * u - 8 * u * u * u),
          fy = phase < 0.5 ? 0 : -6 * Math.sin(Math.PI * u) ** 2;
        px += this.walkDir * fx * p.walk * weight;
        py += (fy - p.lift) * p.walk * weight;
      }
    }
    py +=
      this.model.ghost *
      (0.8 + p.wave) *
      (1 - 0.7 * p.rest) *
      clamp((y + 45) / 45) *
      Math.sin(this.clock * 2 + x / 42);
    return [round(px), round(py)];
  }
  frame() {
    const points = [];
    for (let i = 0; i < this.model.outline.length; i += 2)
      points.push(
        this.deform(this.model.outline[i], this.model.outline[i + 1]),
      );
    let d = `M${points[0].join(" ")}`;
    for (let i = 1; i < points.length; i += 3)
      d += ` C${points
        .slice(i, i + 3)
        .flat()
        .join(" ")}`;
    const p = this.p,
      h = clamp(
        (this.model.faceY - this.model.outline[1]) / -this.model.outline[1],
      );
    const face = [
      round(p.lean * clamp(-this.model.faceY / 200) + p.lag * h ** 1.3),
      round(this.model.faceY * p.sy + p.lift + p.faceExtra),
    ];
    const w = 7 - 2 * p.rest,
      y = 32 + 3 * p.rest,
      cy = 43 - 6 * p.rest,
      b = (y + cy) / 2,
      e = y + (cy - y) / 3;
    const closed = [
      [-w, y],
      [(-2 * w) / 3, e],
      [-w / 3, b],
      [0, b],
      [w / 3, b],
      [(2 * w) / 3, e],
      [w, y],
      [(2 * w) / 3, e],
      [w / 3, b],
      [0, b],
      [-w / 3, b],
      [(-2 * w) / 3, e],
      [-w, y],
    ];
    const expression = Math.max(p.yawn, p.surprise);
    const rx = mix(
        3.7,
        { mochi: 8, pillow: 9.5, ricecake: 6, ghost: 10 }[this.form],
        p.surprise,
      ),
      ry = mix(
        6.5,
        { mochi: 10, pillow: 6, ricecake: 8.5, ghost: 7.5 }[this.form],
        p.surprise,
      ),
      k = 0.5523,
      open = [
        [-rx, 35],
        [-rx, 35 + k * ry],
        [-k * rx, 35 + ry],
        [0, 35 + ry],
        [k * rx, 35 + ry],
        [rx, 35 + k * ry],
        [rx, 35],
        [rx, 35 - k * ry],
        [k * rx, 35 - ry],
        [0, 35 - ry],
        [-k * rx, 35 - ry],
        [-rx, 35 - k * ry],
        [-rx, 35],
      ];
    const mouth = closed.map((v, i) =>
      v.map((x, j) => round(mix(x, open[i][j], expression))),
    );
    let md = `M${mouth[0].join(" ")}`;
    for (let i = 1; i < mouth.length; i += 3)
      md += ` C${mouth
        .slice(i, i + 3)
        .flat()
        .join(" ")}`;
    const air = Math.max(0, this.floor - this.y) / Math.max(1, this.scale);
    return {
      d: d + " Z",
      face,
      mouth: md + " Z",
      mouthStroke: round(3.8 - 3 * expression),
      expression: round(p.surprise),
      brows:
        this.form === "ricecake"
          ? "M-62-33 L-43-39 L-26-34 M27-34 L44-40 L62-35"
          : this.form === "pillow"
            ? "M-62-34 Q-44-44-26-34 M27-34 Q44-44 62-34"
            : "M-62-34 L-53-42 H-35 L-26-34 M27-34 L36-42 H53 L62-34",
      top: points[0][1],
      deep: p.deep,
      shadowWidth: round(
        105 *
          p.sx *
          (1 - 0.35 * clamp(air / 130)) *
          (1 - 0.2 * this.model.ghost),
      ),
      shadowOpacity: round(
        0.48 - 0.22 * clamp(air / 130) - 0.1 * this.model.ghost,
      ),
    };
  }
}
