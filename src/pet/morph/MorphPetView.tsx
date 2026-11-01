import {
  useEffect,
  useImperativeHandle,
  useLayoutEffect,
  useRef,
  type Ref,
} from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import type { GifDisplayState } from "../figure2d";
import { setGifHitPolygon, type GifPoint } from "../gifGeometry";
import { Companion, type FormKey } from "./engine";
import { MorphSignature } from "./MorphSignature";
import {
  createDesktopMotion,
  nativeMotionHost,
  morphEnvelope,
  type MotionHost,
} from "./desktopMotion";
import { loadMorphPreferences, saveMorphPreferences } from "./preferences";

export interface MorphPetHandle {
  poke(): void;
  drag(held: boolean): void;
  hold(held: boolean): void;
  form(form: FormKey): void;
  walk(): void;
  sleep(): void;
  autonomy(): void;
  snapshot(): {
    form: FormKey;
    auto: boolean;
    sleeping: boolean;
    walking: boolean;
  };
}
interface Props {
  readonly width: number;
  readonly state: GifDisplayState;
  readonly visible: boolean;
  readonly leaving: boolean;
  readonly api: Ref<MorphPetHandle>;
  readonly onLoaded: () => void;
  readonly onError: (message: string | null) => void;
  readonly host?: MotionHost;
}
function polygon(engine: Companion): GifPoint[] {
  const a = engine.model.outline,
    points: GifPoint[] = [];
  for (let i = 2; i < a.length; i += 6) {
    const x = i === 2 ? a[0] : a[i - 2],
      y = i === 2 ? a[1] : a[i - 1];
    for (let step = 0; step < 5; step++) {
      const t = step / 5,
        u = 1 - t;
      const p = engine.deform(
        u * u * u * x +
          3 * u * u * t * a[i] +
          3 * u * t * t * a[i + 2] +
          t * t * t * a[i + 4],
        u * u * u * y +
          3 * u * u * t * a[i + 1] +
          3 * u * t * t * a[i + 3] +
          t * t * t * a[i + 5],
      );
      points.push([((p[0] + 180) * 240) / 360, ((p[1] + 280) * 240) / 320]);
    }
  }
  return points;
}
export function MorphPetView({
  width,
  state,
  visible,
  leaving,
  api,
  onLoaded,
  onError,
  host,
}: Props) {
  const svg = useRef<SVGSVGElement>(null),
    body = useRef<SVGPathElement>(null),
    face = useRef<SVGGElement>(null),
    mouth = useRef<SVGPathElement>(null),
    brows = useRef<SVGPathElement>(null),
    shadow = useRef<SVGEllipseElement>(null),
    signs = useRef<SVGGElement>(null),
    hit = useRef<HTMLSpanElement>(null);
  const current = useRef<{
    engine: Companion;
    motion: ReturnType<typeof createDesktopMotion>;
    auto: boolean;
    held: boolean;
    update: () => void;
  } | null>(null);
  const inputs = useRef({ state, visible, leaving });
  useLayoutEffect(() => {
    inputs.current = { state, visible, leaving };
  }, [state, visible, leaving]);
  useImperativeHandle(
    api,
    () => ({
      poke() {
        const c = current.current;
        if (c && inputs.current.state === "idle") c.engine.poke();
      },
      drag(value) {
        current.current?.motion.drag(value);
      },
      hold(value) {
        const c = current.current;
        if (c) {
          c.held = value;
          c.engine.input();
          if (value) c.engine.stopWalk();
        }
      },
      form(form) {
        const c = current.current;
        if (c) {
          c.engine.selectForm(form);
          saveMorphPreferences({ form, auto: c.auto });
        }
      },
      walk() {
        const c = current.current;
        if (!c || inputs.current.state !== "idle") return;
        c.engine.state === "walking"
          ? c.engine.stopWalk()
          : c.engine.startWalk();
      },
      sleep() {
        const c = current.current;
        if (!c || inputs.current.state !== "idle") return;
        ["drowsy", "asleep"].includes(c.engine.state)
          ? c.engine.wake()
          : c.engine.sleep();
      },
      autonomy() {
        const c = current.current;
        if (c) {
          c.auto = !c.auto;
          c.engine.input();
          if (!c.auto) c.engine.stopWalk();
          saveMorphPreferences({ form: c.engine.form, auto: c.auto });
        }
      },
      snapshot() {
        const c = current.current;
        return {
          form: c?.engine.pendingForm ?? c?.engine.form ?? "mochi",
          auto: c?.auto ?? true,
          sleeping: !!c && ["asleep", "drowsy"].includes(c.engine.state),
          walking: c?.engine.state === "walking",
        };
      },
    }),
    [],
  );
  useEffect(() => {
    let disposed = false,
      frame = 0,
      last: number | null = null,
      nextFrame: number | null = null;
    const preferences = loadMorphPreferences();
    const engine = new Companion({
      size: width,
      form: preferences.form,
      auto: preferences.auto,
    });
    const fail = (error: unknown) => {
      if (!disposed)
        onError(error instanceof Error ? error.message : String(error));
    };
    const motion = createDesktopMotion(
      engine,
      host ?? nativeMotionHost(),
      fail,
    );
    const instance = {
      engine,
      motion,
      auto: preferences.auto,
      held: false,
      update,
    };
    current.current = instance;
    const pet = getCurrentWindow();
    let chatVisible = true;
    let chatChanged = false;
    const setChatVisible = (value: boolean) => {
      if (disposed) return;
      chatVisible = value;
      engine.input();
      if (value) engine.stopWalk();
    };
    const chatSubscription = listen<boolean>(
      "deskmate://chat-visibility",
      (event) => {
        chatChanged = true;
        setChatVisible(event.payload);
      },
    );
    void chatSubscription
      .then(() => invoke<boolean>("get_chat_visibility"))
      .then((value) => {
        if (!chatChanged) setChatVisible(value === true);
      })
      .catch(fail);
    const subscriptions = [
      chatSubscription,
      pet.onMoved((event) => motion.moved(event.payload)),
      pet.onResized(() => {
        void motion.sync().catch(fail);
      }),
      pet.onScaleChanged(() => {
        void motion.sync().catch(fail);
      }),
    ];
    for (const subscription of subscriptions) void subscription.catch(fail);
    function draw() {
      const f = engine.frame();
      body.current?.setAttribute("d", f.d);
      face.current?.setAttribute("transform", `translate(${f.face.join(" ")})`);
      mouth.current?.setAttribute("d", f.mouth);
      mouth.current?.setAttribute("stroke-width", String(f.mouthStroke));
      brows.current?.setAttribute("d", f.brows);
      brows.current?.setAttribute("opacity", String(f.expression));
      shadow.current?.setAttribute("rx", String(f.shadowWidth));
      shadow.current?.setAttribute("opacity", String(f.shadowOpacity));
      signs.current?.setAttribute("opacity", String(f.deep * 0.65));
      signs.current?.setAttribute("transform", `translate(80 ${f.top - 10})`);
      if (svg.current) {
        svg.current.dataset.motion = engine.state;
        svg.current.dataset.form = engine.form;
      }
      if (hit.current) setGifHitPolygon(hit.current, polygon(engine));
    }
    function animate(stamp: number) {
      frame = 0;
      if (
        disposed ||
        document.hidden ||
        !inputs.current.visible ||
        inputs.current.leaving
      )
        return;
      if (nextFrame === null || stamp + 1e-6 >= nextFrame) {
        const interval = 1000 / 60;
        nextFrame ??= stamp;
        nextFrame +=
          Math.max(1, Math.floor((stamp - nextFrame + 1e-6) / interval) + 1) *
          interval;
        engine.auto =
          instance.auto &&
          !instance.held &&
          !chatVisible &&
          inputs.current.state === "idle";
        if (!instance.held || engine.state === "dragging")
          motion.tick(
            last === null ? 0 : Math.min(0.05, (stamp - last) / 1000),
          );
        last = stamp;
        draw();
      }
      frame = requestAnimationFrame(animate);
    }
    function reconcile() {
      const active =
        !document.hidden && inputs.current.visible && !inputs.current.leaving;
      motion.setActive(active);
      last = null;
      nextFrame = null;
      if (active && !frame) frame = requestAnimationFrame(animate);
      else if (!active && frame) {
        cancelAnimationFrame(frame);
        frame = 0;
      }
    }
    const changed = () => reconcile();
    document.addEventListener("visibilitychange", changed);
    // Visibility and activity are delivered without rebuilding the engine or resetting a morph.
    function update() {
      if (inputs.current.state !== "idle") {
        engine.stopWalk();
        engine.input();
        if (["asleep", "drowsy"].includes(engine.state)) engine.wake();
      }
      reconcile();
    }
    draw();
    void motion
      .sync()
      .then(() => {
        if (!disposed) {
          onError(null);
          onLoaded();
          reconcile();
        }
      })
      .catch(fail);
    return () => {
      disposed = true;
      cancelAnimationFrame(frame);
      motion.dispose();
      current.current = null;
      document.removeEventListener("visibilitychange", changed);
      for (const subscription of subscriptions)
        void subscription
          .then((stop) => stop())
          .catch((error) =>
            console.error("Morph listener cleanup failed", error),
          );
    };
  }, [width, host, onLoaded, onError]);
  useEffect(() => {
    current.current?.update();
  }, [state, visible, leaving]);
  const scale = width / 270;
  const placement = {
    position: "absolute" as const,
    left: "50%",
    bottom: width * morphEnvelope.bottom - 40 * scale,
    width: 360 * scale,
    height: 320 * scale,
    transform: "translateX(-50%)",
    pointerEvents: "none" as const,
  };
  return (
    <>
      <svg
        ref={svg}
        viewBox="-180 -280 360 320"
        aria-label="百变小著桌面伙伴"
        data-activity={state}
        style={{
          ...placement,
          opacity: visible && !leaving ? 1 : 0,
          transition: leaving ? "opacity 300ms linear" : "none",
        }}
      >
        <ellipse ref={shadow} cx="0" cy="9" ry="8" fill="#c8b59b" />
        <path
          ref={body}
          fill="#fffdf5"
          stroke="#282725"
          strokeWidth="4.5"
          strokeLinejoin="round"
        />
        <g ref={face}>
          <MorphSignature />
          <path
            ref={mouth}
            data-morph-mouth="true"
            fill="#282725"
            stroke="#282725"
            strokeLinejoin="round"
          />
          <path
            ref={brows}
            data-morph-expression="true"
            fill="none"
            stroke="#282725"
            strokeWidth="3.5"
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        </g>
        <g ref={signs} fill="#a79b86" fontSize="15">
          <text>z</text>
          <text x="18" y="-18" fontSize="12">
            z
          </text>
        </g>
        {state !== "idle" && state !== "sleep" && state !== "leaving" && (
          <text
            x="0"
            y="-246"
            textAnchor="middle"
            fill="#282725"
            stroke="#fffdf5"
            strokeWidth="5"
            paintOrder="stroke"
            fontSize="21"
            aria-label={state}
          >
            {state === "error"
              ? "!"
              : state === "success"
                ? "✓"
                : state === "talking"
                  ? "♫"
                  : "···"}
          </text>
        )}
      </svg>
      <span
        ref={hit}
        className="gif-pet-image"
        data-hit-disabled={!visible || leaving}
        style={placement}
      />
    </>
  );
}
