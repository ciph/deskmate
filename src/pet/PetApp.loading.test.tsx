import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { StrictMode } from "react";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { PetMood } from "../lib/petState";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { manualClock } from "../testing/petSleepClock";
import { PetRenderer } from "./PetRenderer";
import { GlbPetView } from "./GlbPetView";
import PetApp from "./PetApp";
import type { Settings } from "../lib/settings";
import type { LoadedGifPersona } from "./gifAssets";
import { parseFigure2dConfig } from "./figure2d";
import config from "../../public/personas/xiaoxiongchong/figure2d.json";

const originalRenderer = PetRenderer;
const gifSettings = legacySettingsFixture({ personaId: "xiaoxiongchong", mouseFollow: false });
const glbSettings = { ...gifSettings, personaId: "xiaozhu" };
const pixel = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
const gifData: LoadedGifPersona = { config: parseFigure2dConfig(config), urls: { idle: pixel, thinking: pixel, working: pixel, talking: pixel, success: pixel, error: pixel, leaving: pixel } };
const gifLoad = async () => gifData;
const glbModule = { GlbPetView };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { promise, resolve, reject };
}
let initialSettings: Settings | Promise<Settings> = gifSettings;
let constructError = false;
let loadModel: (id: string) => Promise<void> = async () => {};
const instances: GpuBoundary[] = [];
const commands: string[] = [];
class GpuBoundary {
  disposed = false;
  constructor(private readonly canvas: HTMLCanvasElement) {
    if (constructError) throw new Error("GPU temporarily unavailable");
    instances.push(this);
  }
  async load(id: string) { this.canvas.dataset.persona = id; await loadModel(id); }
  setMood(mood: PetMood) { this.canvas.dataset.mood = mood; }
  setVisible(visible: boolean) { this.canvas.dataset.visible = String(visible); }
  setRenderTuning() {}
  setMouseFollowEnabled() {}
  dispose() { this.disposed = true; }
}
beforeEach(() => {
  initialSettings = gifSettings; constructError = false; loadModel = async () => {}; instances.length = 0; commands.length = 0;
  restoreTauriModuleFixture(); mockWindows("pet");
  mock.module("./PetRenderer", () => ({ PetRenderer: GpuBoundary }));
  mockIPC(command => {
    commands.push(command);
    if (command === "get_settings") return initialSettings;
    if (command === "get_pet_visibility" || command === "plugin:window|is_visible") return true;
    if (command === "plugin:window|cursor_position") return { x: -100, y: -100 };
    if (command === "plugin:window|outer_position") return { x: 0, y: 0 };
    if (command === "plugin:window|scale_factor") return 1;
    return null;
  }, { shouldMockEvents: true });
});
afterEach(async () => { cleanup(); await act(async () => {}); clearMocks(); mock.module("./PetRenderer", () => ({ PetRenderer: originalRenderer })); });
const change = async (settings: Settings) => act(async () => { await emit("deskmate://settings-changed", settings); });

test("settings, GIF and rig2d startup do not request the GLB module", async () => {
  const settings = deferred<Settings>(); initialSettings = settings.promise;
  const glbLoad = mock(async () => glbModule);
  const view = render(<PetApp gifLoad={gifLoad} glbLoad={glbLoad} />);
  await act(async () => {}); expect(glbLoad).toHaveBeenCalledTimes(0);
  await act(async () => { settings.resolve(gifSettings); });
  expect(view.container.querySelector("img.gif-pet-image")).not.toBeNull();
  expect(glbLoad).toHaveBeenCalledTimes(0);
  await change({ ...gifSettings, personaId: "baobao" });
  expect(view.getByLabelText("包包桌面伙伴")).not.toBeNull();
  expect(glbLoad).toHaveBeenCalledTimes(0);
  expect(instances).toHaveLength(0);
});

test("2D to 3D waits for module and assets, preserving the current mood and sleep clock", async () => {
  const module = deferred<typeof glbModule>(); const model = deferred<void>();
  const glbLoad = mock(() => module.promise); loadModel = () => model.promise;
  const time = manualClock(); const view = render(<PetApp gifLoad={gifLoad} glbLoad={glbLoad} clock={time.clock} />);
  await act(async () => {}); await change(glbSettings);
  expect(glbLoad).toHaveBeenCalledTimes(1);
  expect(view.getByRole("status").textContent).toContain("正在加载 3D 桌宠");
  expect(view.container.querySelector("canvas")).toBeNull();
  await act(async () => { await emit("deskmate://pet-activity", { type: "start", sessionId: "s", requestId: "r", producerEpoch: 1, sequence: 1, eventId: "1:1" }); module.resolve(glbModule); });
  expect(view.container.querySelector("canvas")?.dataset.mood).toBe("thinking");
  expect(view.getByRole("status")).not.toBeNull();
  await act(async () => { await emit("deskmate://pet-activity", { type: "success", sessionId: "s", requestId: "r", producerEpoch: 1, sequence: 2, eventId: "1:2" }); });
  act(() => time.advance(90000));
  expect(view.container.querySelector(".pet-root")?.getAttribute("data-sleep-state")).toBe("awake");
  await act(async () => { model.resolve(); });
  expect(view.queryByRole("status")).toBeNull();
  act(() => time.advance(59999));
  expect(view.container.querySelector(".pet-root")?.getAttribute("data-sleep-state")).toBe("awake");
  act(() => time.advance(1));
  expect(view.container.querySelector(".pet-root")?.getAttribute("data-sleep-state")).toBe("sleep");
});

for (const leave of ["2D", "unmount"]) test(`late module success cannot mount after ${leave}`, async () => {
  initialSettings = glbSettings;
  const module = deferred<typeof glbModule>(); const glbLoad = mock(() => module.promise);
  const view = render(<PetApp gifLoad={gifLoad} glbLoad={glbLoad} />); await act(async () => {});
  expect(glbLoad).toHaveBeenCalledTimes(1);
  if (leave === "2D") await change(gifSettings); else view.unmount();
  await act(async () => { module.resolve(glbModule); });
  expect(instances).toHaveLength(0); expect(view.container.querySelector("canvas")).toBeNull();
  expect(view.queryByRole("alert")).toBeNull();
});

test("module failure offers an independent retry button without opening chat or dragging", async () => {
  initialSettings = glbSettings; const first = deferred<typeof glbModule>();
  const glbLoad = mock(() => first.promise).mockImplementationOnce(() => first.promise).mockImplementation(() => Promise.resolve(glbModule));
  const view = render(<PetApp glbLoad={glbLoad} />); await act(async () => {});
  expect(glbLoad).toHaveBeenCalledTimes(1);
  await act(async () => { first.reject(new Error("chunk fetch failed")); });
  expect(view.getByRole("alert").textContent).toContain("桌宠加载失败");
  expect(view.queryByRole("status")).toBeNull();
  const retry = view.getByRole("button", { name: "重试" });
  expect(retry.closest(".pet-interaction")).toBeNull();
  await act(async () => { fireEvent.mouseDown(retry, { button: 0 }); fireEvent.mouseMove(retry, { screenX: 20 }); fireEvent.mouseUp(retry, { button: 0 }); fireEvent.click(retry); });
  expect(glbLoad).toHaveBeenCalledTimes(2);
  expect(view.queryByRole("alert")).toBeNull(); expect(view.queryByRole("status")).toBeNull();
  expect(view.getByLabelText("3D desktop pet")).not.toBeNull();
  expect(commands.filter(command => command === "toggle_chat" || command.includes("start_dragging"))).toEqual([]);
});

for (const failure of ["constructor", "asset"]) test(`retry creates a clean renderer after ${failure} failure`, async () => {
  initialSettings = glbSettings; constructError = failure === "constructor";
  if (failure === "asset") loadModel = async () => { throw new Error("model unavailable"); };
  const glbLoad = mock(async () => glbModule);
  const view = render(<PetApp glbLoad={glbLoad} />); await act(async () => {});
  expect(view.getByRole("alert")).not.toBeNull(); const before = instances[0];
  constructError = false; loadModel = async () => {};
  await act(async () => { fireEvent.click(view.getByRole("button", { name: "重试" })); });
  expect(glbLoad).toHaveBeenCalledTimes(1);
  expect(view.queryByRole("alert")).toBeNull(); expect(view.queryByRole("status")).toBeNull();
  expect(view.container.querySelector("canvas")?.dataset.persona).toBe("xiaozhu");
  if (before) expect(before.disposed).toBe(true);
});

test("current role wins while module or previous model completion arrives late", async () => {
  initialSettings = glbSettings; const module = deferred<typeof glbModule>();
  const firstModel = deferred<void>(); const lastModel = deferred<void>();
  loadModel = id => id === "xiaozhu-nidaime" ? firstModel.promise : lastModel.promise;
  const glbLoad = mock(() => module.promise);
  const view = render(<StrictMode><PetApp glbLoad={glbLoad} /></StrictMode>); await act(async () => {});
  await change({ ...glbSettings, personaId: "xiaozhu-nidaime" });
  await act(async () => { module.resolve(glbModule); });
  expect(view.container.querySelector("canvas")?.dataset.persona).toBe("xiaozhu-nidaime");
  await change({ ...glbSettings, personaId: "xiaozhu-sandaime" });
  const canvas = view.container.querySelector("canvas");
  await act(async () => { firstModel.resolve(); });
  expect(view.getByRole("status")).not.toBeNull();
  expect(view.container.querySelector("canvas")).toBe(canvas);
  expect(canvas?.dataset.persona).toBe("xiaozhu-sandaime");
  await act(async () => { lastModel.resolve(); });
  expect(view.queryByRole("status")).toBeNull();
  expect(instances.slice(0, -1).every(renderer => renderer.disposed)).toBe(true);
});

test("late module failure cannot replace a recovered 2D pet with an error", async () => {
  initialSettings = glbSettings; const module = deferred<typeof glbModule>();
  const glbLoad = mock(() => module.promise);
  const view = render(<PetApp gifLoad={gifLoad} glbLoad={glbLoad} />); await act(async () => {});
  expect(glbLoad).toHaveBeenCalledTimes(1);
  await change(gifSettings);
  const image = view.container.querySelector("img.gif-pet-image");
  await act(async () => { module.reject(new Error("late chunk failure")); });
  expect(view.container.querySelector("img.gif-pet-image")).toBe(image);
  expect(view.queryByRole("alert")).toBeNull(); expect(view.queryByRole("status")).toBeNull();
  expect(instances).toHaveLength(0);
});

test("rapid 3D to 2D to 3D ignores the first module result and reuses only a successful module", async () => {
  initialSettings = glbSettings;
  const first = deferred<typeof glbModule>(); const second = deferred<typeof glbModule>();
  let attempt = 0; const glbLoad = mock(() => ++attempt === 1 ? first.promise : second.promise);
  const view = render(<PetApp gifLoad={gifLoad} glbLoad={glbLoad} />); await act(async () => {});
  await change(gifSettings); await change({ ...glbSettings, personaId: "xiaozhu-nidaime" });
  await act(async () => { first.resolve(glbModule); });
  expect(instances).toHaveLength(0); expect(view.getByRole("status")).not.toBeNull();
  await act(async () => { second.resolve(glbModule); });
  expect(view.container.querySelector("canvas")?.dataset.persona).toBe("xiaozhu-nidaime");
  const previous = instances[0];
  await change(gifSettings); await change({ ...glbSettings, personaId: "xiaozhu-sandaime" });
  expect(glbLoad).toHaveBeenCalledTimes(2);
  expect(previous?.disposed).toBe(true);
  expect(view.container.querySelector("canvas")?.dataset.persona).toBe("xiaozhu-sandaime");
  expect(view.queryByRole("status")).toBeNull();
});


test("physical visibility suspends and restores the loaded GLB without reloading assets", async () => {
  initialSettings = glbSettings;
  const view = render(<PetApp glbLoad={async () => glbModule} />);
  await act(async () => {});
  const canvas = view.getByLabelText("3D desktop pet");
  const instance = instances[0];
  expect(canvas.getAttribute("data-visible")).toBe("true");
  await act(async () => { await emit("deskmate://pet-native-visibility", false); });
  expect(canvas.getAttribute("data-visible")).toBe("false");
  await act(async () => { await emit("deskmate://pet-native-visibility", true); });
  expect(canvas.getAttribute("data-visible")).toBe("true");
  expect(instances).toHaveLength(1);
  expect(instances[0]).toBe(instance);
  expect(view.getByLabelText("3D desktop pet")).toBe(canvas);
});
