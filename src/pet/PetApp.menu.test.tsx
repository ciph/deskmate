import { afterEach, beforeEach, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { MENU_RESULT, type RadialModel } from "./menu/model";
import type { AiUsage, Settings } from "../lib/settings";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { setGifHitPolygon } from "./gifGeometry";
import PetApp from "./PetApp";
import { parseFigure2dConfig } from "./figure2d";
import type { LoadedGifPersona } from "./gifAssets";
import config from "../../public/personas/xiaoxiongchong/figure2d.json";

const pixel = "data:image/gif;base64,R0lGODlhAQABAIAAAAAAAP///yH5BAEAAAAALAAAAAABAAEAAAIBRAA7";
const gifLoad = async (): Promise<LoadedGifPersona> => ({ config: parseFigure2dConfig(config),
  urls: { idle: pixel, thinking: pixel, working: pixel, talking: pixel, success: pixel, error: pixel, leaving: pixel } });

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { promise, resolve };
}
let settings: Settings;
let opening: Promise<void> | null = null;
let usage: AiUsage | Promise<AiUsage>;
const usageProviders: string[] = [];
const commands: string[] = [];
const models: RadialModel[] = [];
const cancellations: string[] = [];
beforeEach(() => {
  localStorage.clear();
  settings = legacySettingsFixture({ personaId: "xiaozhu-morph" });
  opening = null; commands.length = 0; models.length = 0; cancellations.length = 0;
  usageProviders.length = 0;
  usage = { remainingCny: 27260000, limitCny: 30000000, remainingPct: 91,
    daysUntilReset: 3, todayCostCny: 100, todayRequests: 1, topModels: [] };
  mockWindows("pet");
  mockIPC((command, raw) => {
    commands.push(command);
    const args = raw as { menu: RadialModel; id: string; providerId: string };
    if (command === "fetch_ai_usage") { usageProviders.push(args.providerId); return usage; }
    if (command === "get_settings") return settings;
    if (command === "get_pet_visibility" || command === "plugin:window|is_visible") return true;
    if (command === "plugin:window|cursor_position") return { x: -100, y: -100 };
    if (command === "plugin:window|outer_position") return { x: 0, y: 0 };
    if (command === "plugin:window|outer_size") return { width: 320, height: 420 };
    if (command === "plugin:window|scale_factor") return 1;
    if (command === "plugin:window|current_monitor") return { name: "test", position: { x: 0, y: 0 }, size: { width: 1200, height: 800 }, scaleFactor: 1, workArea: { position: { x: 0, y: 0 }, size: { width: 1200, height: 760 } } };
    if (command === "open_pet_radial_menu" || command === "update_pet_radial_menu") { models.push(args.menu); return command === "open_pet_radial_menu" ? opening : null; }
    if (command === "finish_pet_radial_menu") cancellations.push(args.id);
    return null;
  }, { shouldMockEvents: true });
});
afterEach(async () => { cleanup(); await act(async () => {}); clearMocks(); });
async function setup() {
  const view = render(<PetApp gifLoad={gifLoad} />);
  await act(async () => {});
  const image = view.container.querySelector(".gif-pet-image")!;
  setGifHitPolygon(image, null);
  image.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  const button = view.container.querySelector<HTMLButtonElement>(".pet-interaction")!;
  return { view, button };
}
const rightClick = (button: HTMLButtonElement) => act(async () => { fireEvent.contextMenu(button, { clientX: 50, clientY: 50 }); });
const choose = (action: string | null, id = models.at(-1)!.id) => act(async () => { await emit(MENU_RESULT, { id, action }); });

test("right click opens six categories with a single AI entry and never fetches usage", async () => {
  const { button } = await setup();
  expect(commands).not.toContain("fetch_ai_usage");
  await rightClick(button);
  expect(models[0].items.map(item => item.id)).toEqual(["chat", "actions", "forms", "appearance", "ai-usage", "more"]);
  expect(models[0].items.find(item => item.id === "forms")!.children!.map(item => item.id)).toEqual(["morph-mochi", "morph-pillow", "morph-ricecake", "morph-ghost"]);
  expect(JSON.stringify(models[0])).not.toContain("apiKey");
  await rightClick(button);
  expect(commands.filter(command => command === "open_pet_radial_menu")).toHaveLength(1);
  expect(commands).not.toContain("fetch_ai_usage");
  expect("usage" in models[0]).toBe(false);
  expect(models[0].anchor).toEqual({ left: 0, top: 0, width: 100, height: 100 });
  const id = models[0].id;
  await choose("chat", id); await choose("chat", id);
  expect(commands.filter(command => command === "toggle_chat")).toHaveLength(1);
});

test("classic 2D and milk remain selectable and selected appearance is reflected on reopening", async () => {
  const { button } = await setup(); await rightClick(button); await choose("morph-appearance-classic");
  expect(document.querySelector("[data-appearance=classic] [data-pixel-glasses]")).not.toBeNull();
  await rightClick(button);
  expect(models.at(-1)!.items.find(item => item.id === "appearance")!.children!.find(item => item.id === "morph-appearance-classic")!.checked).toBe(true);
  await choose("morph-appearance-milk");
  expect(document.querySelector("[data-appearance=milk] [data-milk-glasses]")).not.toBeNull();
});

test("AI entry opens a usage cloud, fetches only the active provider and never opens settings", async () => {
  const { button } = await setup(); await rightClick(button);
  await choose("ai-usage");
  expect(commands).not.toContain("open_ai_model_settings");
  expect(commands).not.toContain("open_settings");
  expect(usageProviders).toEqual([settings.activeProviderId]);
  const cloud = models.at(-1)!;
  expect(cloud.items).toEqual([]);
  expect(cloud.usage).toEqual({ label: "本周剩余", percent: 91, amount: "¥2726.00", note: "" });
  expect(JSON.stringify(cloud)).not.toContain("configured-key");
  expect(JSON.stringify(cloud)).not.toContain("provider-kuro");
  await choose(null);
  await rightClick(button);
  expect(models.at(-1)!.usage).toBeUndefined();
});

test("unconfigured AI opens a dismissible status cloud without requesting usage", async () => {
  const { button } = await setup(); await rightClick(button);
  await act(async () => { await emit("deskmate://settings-changed", { ...settings, providers: [], activeProviderId: "" }); });
  expect(models.at(-1)!.items.filter(item => item.id === "ai-usage")).toHaveLength(1);
  expect(JSON.stringify(models.at(-1))).not.toContain("provider-kuro");
  await choose("ai-usage");
  expect(commands).not.toContain("open_ai_model_settings");
  expect(commands).not.toContain("fetch_ai_usage");
  expect(models.at(-1)!.usage?.note).toBe("填写 API Key 后即可查看用量");
  await choose(null);
});

test("closing a loading cloud ignores its late response and reopening fetches afresh", async () => {
  const pending = deferred<AiUsage>(); const result = usage as AiUsage; usage = pending.promise;
  const { button } = await setup(); await rightClick(button); await choose("ai-usage");
  expect(models.at(-1)!.usage?.note).toBe("正在读取 AI 用量…");
  await choose(null);
  const count = models.length;
  await act(async () => { pending.resolve(result); });
  expect(models).toHaveLength(count);
  usage = result;
  await rightClick(button); await choose("ai-usage");
  expect(usageProviders).toHaveLength(2);
  expect(models.at(-1)!.usage?.percent).toBe(91);
});

test("changing provider closes the cloud and ignores stale usage", async () => {
  const pending = deferred<AiUsage>(); const result = usage as AiUsage; usage = pending.promise;
  const { button } = await setup(); await rightClick(button); await choose("ai-usage");
  const id = models.at(-1)!.id;
  await act(async () => { await emit("deskmate://settings-changed", { ...settings, activeProviderId: "other" }); });
  expect(cancellations).toContain(id);
  const count = models.length;
  await act(async () => { pending.resolve(result); });
  expect(models).toHaveLength(count);
});

test("failed usage lookup leaves a closable error cloud without a settings redirect", async () => {
  const { button } = await setup(); await rightClick(button);
  usage = Promise.reject(new Error("status:403"));
  await choose("ai-usage");
  expect(models.at(-1)!.usage?.note).toBe("当前 API Key 没有用量查询权限");
  expect(commands).not.toContain("open_ai_model_settings");
  await choose(null);
});

test("other 2D pets retain chat, usage, and settings; transparent areas do not open a menu", async () => {
  settings = { ...settings, personaId: "xiaoxiongchong" };
  const { button } = await setup();
  await act(async () => { fireEvent.contextMenu(button, { clientX: 200, clientY: 200 }); });
  expect(models).toHaveLength(0);
  await rightClick(button);
  expect(models[0].items.map(item => item.id)).toEqual(["chat", "ai-usage", "more"]);
  await choose("open-widget-settings"); expect(commands).toContain("open_widget_settings");
});

test("unmount during window creation cancels again after creation and ignores late results", async () => {
  const pending = deferred<void>(); opening = pending.promise;
  const { button, view } = await setup(); await rightClick(button);
  const id = models[0].id;
  view.unmount(); await act(async () => { pending.resolve(); });
  expect(cancellations.filter(value => value === id)).toHaveLength(2);
  await choose("chat", id);
  expect(commands).not.toContain("toggle_chat");
  expect(commands).not.toContain("update_pet_radial_menu");
});

test("keyboard menu shortcut opens the same hierarchy without background usage requests", async () => {
  const { button } = await setup();
  await act(async () => { fireEvent.keyDown(button, { key: "F10", shiftKey: true }); });
  expect(models[0].items).toHaveLength(6);
  await choose(null);
  expect(commands).not.toContain("fetch_ai_usage");
});
