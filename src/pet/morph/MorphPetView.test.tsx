import { afterEach, beforeEach, expect, mock, test, spyOn } from "bun:test";
import { createRef } from "react";
import { act, cleanup, render } from "@testing-library/react";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { restoreTauriModuleFixture } from "../../testing/tauriModuleFixture";
import { MorphPetView, type MorphPetHandle } from "./MorphPetView";
import type { DesktopGeometry } from "./desktopMotion";
import { Companion } from "./engine";
import { emit } from "@tauri-apps/api/event";
let timestamp = 0,
  serial = 0;
const callbacks = new Map<number, FrameRequestCallback>();
let raf: ReturnType<typeof spyOn>, cancel: ReturnType<typeof spyOn>;
beforeEach(() => {
  restoreTauriModuleFixture();
  mockWindows("pet");
  mockIPC(() => null, { shouldMockEvents: true });
  localStorage.clear();
  callbacks.clear();
  timestamp = 0;
  raf = spyOn(globalThis, "requestAnimationFrame").mockImplementation(
    (callback) => {
      callbacks.set(++serial, callback);
      return serial;
    },
  );
  cancel = spyOn(globalThis, "cancelAnimationFrame").mockImplementation(
    (id) => {
      callbacks.delete(id);
    },
  );
});
afterEach(() => {
  cleanup();
  clearMocks();
  raf.mockRestore();
  cancel.mockRestore();
});
async function advance(seconds: number) {
  for (let i = 0; i < Math.ceil(seconds * 60); i++)
    await act(async () => {
      timestamp += 1000 / 60;
      const run = [...callbacks.values()];
      callbacks.clear();
      run.forEach((cb) => cb(timestamp));
    });
}
const geometry: DesktopGeometry = {
  position: { x: 300, y: 500 },
  size: { width: 320, height: 242 },
  ratio: 1,
  area: { x: 0, y: 0, width: 1600, height: 1000 },
};
for (const appearance of ["milk", "classic"] as const)
  test(`${appearance}: the ground shadow stays on the desktop, fades in the air and returns on landing`, async () => {
    localStorage.setItem("yume-xiaozhu-morph-v1", JSON.stringify({ appearance, auto: false }));
    const api = createRef<MorphPetHandle>();
    const scale = 160 / 270;
    let native = { ...geometry, position: { x: 300, y: 758 } };
    const move = async (position: { x: number; y: number }) => {
      native = { ...native, position };
      await emit("tauri://move", position);
    };
    const view = render(<MorphPetView width={160} state="idle" visible leaving={false} api={api}
      onLoaded={() => {}} onError={message => { if (message) throw new Error(message); }}
      host={{ read: async () => native, write: move }} />);
    await act(async () => {});
    const shadow = view.container.querySelector(`[data-appearance-layer="${appearance}"] > ellipse`)!;
    const opacity = () => Number(shadow.getAttribute("opacity"));
    const screenY = () => native.position.y + native.size.height - 160 * 0.2 + Number(shadow.getAttribute("cy")) * scale;
    for (const form of ["mochi", "pillow", "ricecake", "ghost"] as const) {
      act(() => api.current!.form(form));
      await advance(1.5);
      const floorY = screenY();
      const restingOpacity = opacity();
      expect(restingOpacity).toBeGreaterThan(0.3);
      act(() => api.current!.drag(true));
      await act(async () => { await move({ x: 420, y: 758 - 10 * scale }); });
      await advance(0.3);
      expect(screenY()).toBeCloseTo(floorY, 3);
      expect(opacity()).toBeGreaterThan(0);
      expect(opacity()).toBeLessThan(restingOpacity);
      await act(async () => { await move({ x: 420, y: 500 }); });
      await advance(0.3);
      expect(screenY()).toBeCloseTo(floorY, 3);
      expect(opacity()).toBe(0);
      act(() => api.current!.drag(false));
      await act(async () => {});
      await advance(0.1);
      expect(opacity()).toBe(0);
      await advance(6);
      expect(view.container.querySelector("svg")?.dataset.motion).toBe("idle");
      expect(opacity()).toBeCloseTo(restingOpacity, 2);
      expect(screenY()).toBeCloseTo(floorY, 3);
    }
  });
for (const hz of [60, 120, 144])
  test(`60 fps keeps its time budget on a ${hz} Hz screen`, async () => {
    const update = spyOn(Companion.prototype, "update");
    try {
      render(
        <MorphPetView
          width={160}
          state="idle"
          visible
          leaving={false}
          api={createRef()}
          onLoaded={() => {}}
          onError={() => {}}
          host={{ read: async () => geometry, write: async () => {} }}
        />,
      );
      await act(async () => {});
      for (let i = 0; i <= hz * 5; i++)
        await act(async () => {
          const run = [...callbacks.values()];
          callbacks.clear();
          run.forEach((callback) => callback((i * 1000) / hz));
        });
      expect(update.mock.calls.length).toBeGreaterThanOrEqual(300);
      expect(update.mock.calls.length).toBeLessThanOrEqual(301);
      expect(update.mock.calls.reduce((sum, [dt]) => sum + dt, 0)).toBeCloseTo(
        5,
        1,
      );
    } finally {
      update.mockRestore();
    }
  });
test("four morphs keep one rigid signature; sleep wakes; hidden rendering stops; preferences survive remount", async () => {
  localStorage.setItem("yume-xiaozhu-morph-v1", JSON.stringify({ appearance: "classic" }));
  const api = createRef<MorphPetHandle>(),
    ready = mock(() => {}),
    error = mock((_message: string | null) => {});
  const host = { read: async () => geometry, write: mock(async () => {}) };
  const props = {
    width: 160,
    state: "idle" as const,
    visible: true,
    leaving: false,
    api,
    onLoaded: ready,
    onError: error,
    host,
  };
  const view = render(<MorphPetView {...props} />);
  await act(async () => {});
  await advance(1);
  expect(ready).toHaveBeenCalledTimes(1);
  expect(error.mock.calls.every(([message]) => message === null)).toBe(true);
  const signature = view.container.querySelector(
    "[data-morph-signature]",
  )!.innerHTML;
  const mouth = view.container.querySelector("[data-morph-mouth]")!;
  const restingMouth = mouth.getAttribute("d");
  act(() => api.current!.drag(true));
  await advance(0.4);
  expect(mouth.getAttribute("d")).not.toBe(restingMouth);
  expect(
    Number(
      view.container
        .querySelector("[data-morph-expression]")
        ?.getAttribute("opacity"),
    ),
  ).toBeGreaterThan(0.9);
  expect(
    view.container.querySelector("[data-morph-signature]")!.innerHTML,
  ).toBe(signature);
  act(() => api.current!.drag(false));
  await advance(5);
  expect(
    Number(
      view.container
        .querySelector("[data-morph-expression]")
        ?.getAttribute("opacity"),
    ),
  ).toBeLessThan(0.01);
  for (const form of ["pillow", "ricecake", "ghost", "mochi"] as const) {
    act(() => api.current!.form(form));
    await advance(1);
    expect(view.container.querySelector("svg")?.dataset.form).toBe(form);
    expect(
      view.container.querySelector("[data-morph-signature]")!.innerHTML,
    ).toBe(signature);
  }
  act(() => api.current!.sleep());
  await advance(4);
  expect(view.container.querySelector("svg")?.dataset.motion).toBe("asleep");
  act(() => api.current!.poke());
  await advance(2);
  expect(view.container.querySelector("svg")?.dataset.motion).toBe("idle");
  act(() => api.current!.form("ghost"));
  await advance(1);
  view.rerender(<MorphPetView {...props} visible={false} />);
  await act(async () => {});
  const writes = host.write.mock.calls.length;
  expect(callbacks.size).toBe(0);
  await advance(2);
  expect(host.write.mock.calls).toHaveLength(writes);
  view.unmount();
  render(<MorphPetView {...props} />);
  await act(async () => {});
  expect(api.current!.snapshot().form).toBe("ghost");
});
test("pixel glasses snap to device pixels while the soft face keeps its smooth offset", async () => {
  localStorage.setItem("yume-xiaozhu-morph-v1", JSON.stringify({ appearance: "classic" }));
  const ratio = Object.getOwnPropertyDescriptor(globalThis, "devicePixelRatio");
  Object.defineProperty(globalThis, "devicePixelRatio", { configurable: true, value: 1.25 });
  const rect = spyOn(SVGElement.prototype, "getBoundingClientRect").mockReturnValue({
    left: 10.3, top: 4.7, width: 213.333, height: 189.63, right: 223.633, bottom: 194.33, x: 10.3, y: 4.7,
    toJSON: () => ({}),
  } as DOMRect);
  try {
    const view = render(
      <MorphPetView width={160} state="idle" visible leaving={false} api={createRef()}
        onLoaded={() => {}} onError={() => {}}
        host={{ read: async () => geometry, write: async () => {} }} />,
    );
    await act(async () => {});
    await advance(0.5);
    const glasses = view.container.querySelector("[data-pixel-glasses]")!;
    const [, , , s] = /translate\((\S+) (\S+)\) scale\((\S+)\)/.exec(glasses.getAttribute("transform") ?? "")!;
    const cell = 6 * Number(s) * (213.333 / 360) * 1.25;
    expect(Math.abs(cell - Math.round(cell))).toBeLessThan(1e-3);
    expect(glasses.getAttribute("shape-rendering")).toBe("crispEdges");
    expect(view.container.querySelector("[data-morph-signature] ellipse")?.getAttribute("transform")).toBeNull();
  } finally {
    rect.mockRestore();
    if (ratio) Object.defineProperty(globalThis, "devicePixelRatio", ratio);
    else Reflect.deleteProperty(globalThis, "devicePixelRatio");
  }
});
test("milk and classic switch without restarting motion, and the selection survives remount", async () => {
  const api = createRef<MorphPetHandle>();
  const ready = mock(() => {});
  const props = { width: 160, state: "idle" as const, visible: true, leaving: false, api,
    onLoaded: ready, onError: () => {}, host: { read: async () => geometry, write: async () => {} } };
  const view = render(<MorphPetView {...props} />);
  await act(async () => {});
  const milk = view.container.querySelector('[data-appearance-layer="milk"]')!;
  const classic = view.container.querySelector('[data-appearance-layer="classic"]')!;
  expect(api.current!.snapshot().appearance).toBe("milk");
  const artwork = milk.querySelector("[data-morph-signature]")!.innerHTML;
  expect(milk.querySelector("[data-morph-expression]")).toBeNull();
  expect(classic.getAttribute("display")).toBe("none");
  for (const form of ["pillow", "ricecake", "ghost", "mochi"] as const) {
    act(() => api.current!.form(form));
    await advance(1);
    expect(milk.querySelector("[data-morph-signature]")!.innerHTML).toBe(artwork);
    expect(milk.querySelector("[data-milk-glasses]")!.getAttribute("transform")).toBeNull();
    expect(milk.querySelector("[shape-rendering=crispEdges]")).toBeNull();
    expect(milk.querySelector("[data-milk-feet]")!.getAttribute("opacity")).toBe(form === "ghost" ? "0" : "1");
    expect(milk.querySelector("[data-morph-expression]")).toBeNull();
  }
  const restingMouth = milk.querySelector("[data-morph-mouth]")!.getAttribute("d");
  act(() => api.current!.drag(true));
  await advance(0.4);
  expect(milk.querySelector("[data-morph-expression]")).toBeNull();
  expect(milk.querySelector("[data-morph-mouth]")!.getAttribute("d")).not.toBe(restingMouth);
  act(() => api.current!.drag(false));
  await advance(5);
  expect(milk.querySelector("[data-morph-mouth]")!.getAttribute("d")).toBe(restingMouth);
  act(() => { api.current!.autonomy(); api.current!.form("ghost"); });
  await advance(0.3);
  const motion = view.container.querySelector("svg")!.dataset.motion;
  act(() => api.current!.appearance("classic"));
  expect(view.container.querySelector("svg")!.dataset.motion).toBe(motion);
  expect(view.container.querySelector("svg")!.dataset.form).toBe("ghost");
  expect(view.container.querySelector("[data-pixel-glasses]")).not.toBeNull();
  expect(view.container.querySelector("[data-morph-expression]")).not.toBeNull();
  expect(milk.getAttribute("opacity")).toBe("1");
  expect(api.current!.snapshot()).toMatchObject({ form: "ghost", auto: false, appearance: "classic" });
  expect(ready).toHaveBeenCalledTimes(1);
  await advance(1);
  expect(milk.getAttribute("display")).toBe("none");
  expect(classic.getAttribute("opacity")).toBe("1");
  view.unmount();
  const remounted = render(<MorphPetView {...props} />);
  await act(async () => {});
  expect(api.current!.snapshot()).toMatchObject({ form: "ghost", auto: false, appearance: "classic" });
  act(() => api.current!.appearance("milk"));
  expect(document.querySelector("[data-milk-glasses] image")).not.toBeNull();
  await advance(0.5);
  expect(remounted.container.querySelector('[data-appearance-layer="classic"]')?.getAttribute("display")).toBe("none");
  expect(remounted.container.querySelector('[data-appearance-layer="milk"] [data-morph-expression]')).toBeNull();
});

test("appearance fades share the live pose, reverse continuously and finish while held", async () => {
  const api = createRef<MorphPetHandle>();
  const ready = mock(() => {});
  const view = render(<MorphPetView width={160} state="idle" visible leaving={false} api={api}
    onLoaded={ready} onError={() => {}} host={{ read: async () => geometry, write: async () => {} }} />);
  await act(async () => {});
  await advance(1);
  const milk = view.container.querySelector('[data-appearance-layer="milk"]')!;
  const classic = view.container.querySelector('[data-appearance-layer="classic"]')!;
  const weight = () => Number(milk.getAttribute("opacity"));
  act(() => { api.current!.drag(true); api.current!.appearance("classic"); });
  expect(weight()).toBe(1);
  await advance(0.2);
  expect(weight()).toBeCloseTo(0.5, 2);
  expect(Number(classic.getAttribute("opacity")) + weight()).toBeCloseTo(1, 6);
  expect(classic.querySelector("[data-morph-mouth]")?.getAttribute("d"))
    .toBe(milk.querySelector("[data-morph-mouth]")?.getAttribute("d"));
  const pose = milk.querySelector("[data-morph-mouth]")!.getAttribute("d");
  act(() => api.current!.appearance("milk"));
  expect(weight()).toBeCloseTo(0.5, 2);
  await advance(0.1);
  expect(weight()).toBeGreaterThan(0.5);
  expect(weight()).toBeLessThan(1);
  expect(milk.querySelector("[data-morph-mouth]")!.getAttribute("d")).not.toBe(pose);
  act(() => api.current!.appearance("milk"));
  await advance(0.3);
  expect(weight()).toBe(1);
  expect(classic.getAttribute("display")).toBe("none");
  act(() => { api.current!.drag(false); api.current!.hold(true); api.current!.appearance("classic"); });
  await act(async () => {});
  await advance(0.5);
  expect(weight()).toBe(0);
  expect(classic.getAttribute("opacity")).toBe("1");
  expect(milk.getAttribute("display")).toBe("none");
  expect(ready).toHaveBeenCalledTimes(1);
  view.unmount();
  expect(callbacks.size).toBe(0);
});

for (const reduce of [false, true])
test(`reduced motion=${reduce}: hidden appearance changes stop rendering and resume at the selected style`, async () => {
  const preference = spyOn(window, "matchMedia").mockReturnValue({ matches: reduce } as MediaQueryList);
  try {
    const api = createRef<MorphPetHandle>();
    const props = { width: 160, state: "idle" as const, visible: true, leaving: false, api,
      onLoaded: () => {}, onError: () => {}, host: { read: async () => geometry, write: async () => {} } };
    const view = render(<MorphPetView {...props} />);
    await act(async () => {});
    await advance(0.1);
    act(() => api.current!.appearance("classic"));
    if (reduce) {
      expect(view.container.querySelector('[data-appearance-layer="classic"]')?.getAttribute("opacity")).toBe("1");
      expect(view.container.querySelector('[data-appearance-layer="milk"]')?.getAttribute("display")).toBe("none");
    } else {
      await advance(0.2);
      expect(Number(view.container.querySelector('[data-appearance-layer="classic"]')?.getAttribute("opacity"))).toBeCloseTo(0.5, 2);
    }
    view.rerender(<MorphPetView {...props} visible={false} />);
    act(() => api.current!.appearance("milk"));
    await advance(1);
    expect(callbacks.size).toBe(0);
    view.rerender(<MorphPetView {...props} visible />);
    await advance(0.5);
    expect(view.container.querySelector('[data-appearance-layer="milk"]')?.getAttribute("opacity")).toBe("1");
  } finally { preference.mockRestore(); }
});

test("opening chat stops walking and pauses autonomy until it closes", async () => {
  const api = createRef<MorphPetHandle>();
  const view = render(
    <MorphPetView
      width={160}
      state="idle"
      visible
      leaving={false}
      api={api}
      onLoaded={() => {}}
      onError={() => {}}
      host={{ read: async () => geometry, write: async () => {} }}
    />,
  );
  await act(async () => {});
  await advance(2);
  act(() => api.current!.walk());
  await advance(1);
  expect(api.current!.snapshot().walking).toBe(true);
  await act(async () => {
    await emit("deskmate://chat-visibility", true);
  });
  await advance(70);
  expect(view.container.querySelector("svg")?.dataset.motion).toBe("idle");
  await act(async () => {
    await emit("deskmate://chat-visibility", false);
  });
  await advance(25);
  expect(api.current!.snapshot().walking).toBe(true);
});

test("real work interrupts walking and sleep, and forbids idle interactions until it finishes", async () => {
  const api = createRef<MorphPetHandle>(),
    ready = () => {},
    error = (message: string | null) => {
      if (message) throw new Error(message);
    };
  const host = { read: async () => geometry, write: async () => {} };
  const props = {
    width: 160,
    state: "idle" as const,
    visible: true,
    leaving: false,
    api,
    onLoaded: ready,
    onError: error,
    host,
  };
  const view = render(<MorphPetView {...props} />);
  await act(async () => {});
  await advance(2);
  act(() => api.current!.sleep());
  await advance(4);
  view.rerender(<MorphPetView {...props} state="working" />);
  await advance(2);
  act(() => {
    api.current!.walk();
    api.current!.sleep();
  });
  await advance(1);
  expect(view.container.querySelector("svg")?.dataset.motion).toBe("idle");
  expect(view.container.querySelector("svg")?.dataset.activity).toBe("working");
});
