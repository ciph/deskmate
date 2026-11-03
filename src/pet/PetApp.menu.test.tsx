import { afterEach, beforeEach, expect, test } from "bun:test";
import { act, cleanup, fireEvent, render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { Channel } from "@tauri-apps/api/core";
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
let popup = deferred<void>();
let usage = deferred<AiUsage>();
let creation: Promise<void> | null;
const commands: string[] = [];
const rows = new Map<number, { id: string; text: string; handler: Channel<string> }>();
const closed = new Set<number>();
beforeEach(() => {
  settings = legacySettingsFixture({ personaId: "xiaozhu-morph" });
  popup = deferred(); usage = deferred(); creation = null; commands.length = 0; rows.clear(); closed.clear();
  mockWindows("pet");
  mockIPC((command, raw) => {
    commands.push(command);
    const args = raw as { kind: string; options: { id?: string; text?: string }; handler: Channel<string>; rid: number; text: string };
    if (command === "get_settings") return settings;
    if (command === "get_pet_visibility" || command === "plugin:window|is_visible") return true;
    if (command === "plugin:window|cursor_position") return { x: -100, y: -100 };
    if (command === "plugin:window|outer_position") return { x: 0, y: 0 };
    if (command === "plugin:window|outer_size") return { width: 320, height: 420 };
    if (command === "plugin:window|scale_factor") return 1;
    if (command === "plugin:window|current_monitor") return { name: "test", position: { x: 0, y: 0 }, size: { width: 1200, height: 800 }, scaleFactor: 1, workArea: { position: { x: 0, y: 0 }, size: { width: 1200, height: 760 } } };
    if (command === "fetch_ai_usage") return usage.promise;
    if (command === "popup_pet_menu") return popup.promise;
    if (command === "plugin:menu|new") {
      const rid = rows.size + 1;
      const id = args.options.id ?? args.kind;
      rows.set(rid, { id, text: args.options.text ?? "", handler: args.handler });
      return creation ? creation.then(() => [rid, id]) : [rid, id];
    }
    if (command === "plugin:menu|set_text") {
      if (closed.has(args.rid)) throw new Error("write after menu closed");
      rows.get(args.rid)!.text = args.text;
    }
    if (command === "plugin:resources|close") closed.add(args.rid);
    return null;
  }, { shouldMockEvents: true });
});
afterEach(async () => { cleanup(); popup.resolve(); await act(async () => {}); clearMocks(); });

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

test("right-click shows usage and existing morph controls without waiting for the account request", async () => {
  const { button } = await setup();
  expect(commands).not.toContain("fetch_ai_usage");
  await rightClick(button);
  expect(commands).toContain("popup_pet_menu");
  expect(commands).not.toContain("plugin:menu|popup");
  expect([...rows.values()].map(row => row.id)).toEqual([
    "pet-ai-usage-0", "pet-ai-usage-1", "pet-ai-usage-2", "pet-ai-usage-3", "Predefined",
    "morph-chat", "morph-mochi", "morph-pillow", "morph-ricecake", "morph-ghost",
    "morph-poke", "morph-walk", "morph-sleep", "morph-auto", "open-widget-settings", "open-pet-settings", "Menu",
  ]);
  await rightClick(button);
  expect(commands.filter(command => command === "popup_pet_menu")).toHaveLength(1);
  await act(async () => { usage.resolve({ kind: "local", localAvailable: true, todayTokens: 88, todayRequests: 1, topModels: [] }); });
  expect(rows.get(2)!.text).toContain("88 tokens");
  const chat = [...rows.values()].find(row => row.id === "morph-chat")!;
  chat.handler.onmessage(chat.id);
  expect(commands).toContain("toggle_chat");
  await act(async () => { popup.resolve(); });
  expect(closed.size).toBe(rows.size);
});

test("settings events update an open menu and pending requests cannot overwrite missing-key state", async () => {
  const { button } = await setup(); await rightClick(button);
  await act(async () => { await emit("deskmate://settings-changed", { ...settings, providers: [], activeProviderId: "" }); });
  expect(rows.get(4)!.text).toBe("配置 AI…");
  await act(async () => { usage.resolve({ kind: "local", localAvailable: true, todayTokens: 99, todayRequests: 1, topModels: [] }); });
  expect(rows.get(2)!.text).toBe("填写 API Key 后即可查看用量");
});

test("other 2D pets share the summary while transparent areas do not open the menu", async () => {
  settings = { ...settings, personaId: "xiaoxiongchong" };
  const { button } = await setup();
  await act(async () => { fireEvent.contextMenu(button, { clientX: 200, clientY: 200 }); });
  expect(commands).not.toContain("fetch_ai_usage");
  await rightClick(button);
  expect(commands).toContain("popup_pet_menu");
  expect([...rows.values()].map(row => row.id)).toContain("open-widget-settings");
  expect([...rows.values()].some(row => row.id === "morph-chat")).toBe(false);
});

test("unmount during asynchronous construction cleans partial resources without opening a popup", async () => {
  const { button, view } = await setup();
  const pending = deferred<void>(); creation = pending.promise;
  await rightClick(button); view.unmount();
  await act(async () => { pending.resolve(); });
  expect(commands).not.toContain("popup_pet_menu");
  expect(closed.size).toBe(rows.size);
});
