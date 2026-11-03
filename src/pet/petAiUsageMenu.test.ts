import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import type { Channel } from "@tauri-apps/api/core";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import type { AiUsage } from "../lib/settings";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { PetAiUsageCache } from "./petAiUsage";
import { appendPetAiUsageMenu, type PetMenuItem } from "./petAiUsageMenu";

const settings = legacySettingsFixture();
const local: AiUsage = { kind: "local", localAvailable: true, todayTokens: 42, todayRequests: 1, topModels: [] };
const rows = new Map<number, { text: string; enabled: boolean; handler: Channel<string> }>();
const commands: string[] = [];
const closed = new Set<number>();
let failAt = -1;
let writes = 0;
let pendingWrite: Promise<void> | null = null;
const report = mock((_error: unknown) => {});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>(done => { resolve = done; });
  return { resolve, promise };
}
const flush = () => new Promise<void>(done => setTimeout(done, 0));
const texts = () => [...rows.values()].map(row => row.text);
beforeEach(() => {
  rows.clear(); commands.length = 0; closed.clear(); writes = 0; failAt = -1; pendingWrite = null; report.mockClear();
  mockIPC((command, raw) => {
    commands.push(command);
    const args = raw as { options: { id?: string; text?: string; enabled?: boolean }; handler: Channel<string>; rid: number; text: string };
    if (command === "plugin:menu|new") {
      if (rows.size === failAt) throw new Error("creation failed");
      const rid = rows.size + 1;
      rows.set(rid, { text: args.options.text ?? "Separator", enabled: args.options.enabled ?? false, handler: args.handler });
      return [rid, args.options.id ?? "separator"];
    }
    if (command === "plugin:menu|set_text") {
      if (closed.has(args.rid)) throw new Error("write after close");
      writes += 1;
      rows.get(args.rid)!.text = args.text;
      return pendingWrite;
    }
    if (command === "plugin:resources|close") closed.add(args.rid);
    return null;
  });
});
afterEach(clearMocks);

test("loading menu does not await the network and details opens the existing AI settings", async () => {
  const response = deferred<AiUsage>();
  const items: PetMenuItem[] = [];
  const menu = await appendPetAiUsageMenu(items, settings, new PetAiUsageCache(() => response.promise), report);
  expect(texts()[1]).toBe("正在读取 AI 用量…");
  expect(items).toHaveLength(5);
  rows.get(4)!.handler.onmessage("pet-ai-usage-3");
  expect(commands).toContain("open_ai_model_settings");
  response.resolve(local); await flush();
  expect(texts().slice(1, 3)).toEqual(["Yume 今日用量  42 tokens · 1 次", "仅统计 Yume 记录到的请求"]);
  await menu.dispose();
  await Promise.all(items.map(item => item.close()));
  expect(closed.size).toBe(5);
  expect(report).not.toHaveBeenCalled();
});

test("closing while a request is in flight prevents writes to released resources", async () => {
  const response = deferred<AiUsage>();
  const items: PetMenuItem[] = [];
  const menu = await appendPetAiUsageMenu(items, settings, new PetAiUsageCache(() => response.promise), report);
  await menu.dispose(); await Promise.all(items.map(item => item.close()));
  const before = writes;
  response.resolve(local); await flush();
  expect(writes).toBe(before);
  expect(report).not.toHaveBeenCalled();
});

test("disposing waits for already submitted text writes before resources can close", async () => {
  const response = deferred<AiUsage>();
  const items: PetMenuItem[] = [];
  const menu = await appendPetAiUsageMenu(items, settings, new PetAiUsageCache(() => response.promise), report);
  const writing = deferred<void>(); pendingWrite = writing.promise;
  response.resolve(local); await flush();
  let disposed = false;
  const done = menu.dispose().then(() => { disposed = true; });
  await flush(); expect(disposed).toBe(false);
  writing.resolve(); await done;
  expect(disposed).toBe(true);
});

test("provider switch updates the open menu and ignores the old account response", async () => {
  const old = deferred<AiUsage>(); const current = deferred<AiUsage>();
  const cache = new PetAiUsageCache(id => id === settings.activeProviderId ? old.promise : current.promise);
  const menu = await appendPetAiUsageMenu([], settings, cache, report);
  const provider = { ...settings.providers[0]!, id: "new", label: "B & C" };
  menu.update({ ...settings, providers: [provider], activeProviderId: provider.id });
  current.resolve(local); await flush();
  expect(texts()[0]).toBe("AI 用量 · B && C");
  old.resolve({ ...local, todayTokens: 999 }); await flush();
  expect(texts()[1]).toContain("42 tokens");
  await menu.dispose();
});

test("stale cache is labelled updating, with localized no-key and error states", async () => {
  let now = 100_000;
  const response = deferred<AiUsage>();
  const fetch = mock(async (): Promise<AiUsage> => local);
  const cache = new PetAiUsageCache(fetch, () => now);
  await cache.read(settings.providers[0]!).refreshed;
  now += 60_000; fetch.mockImplementation(() => response.promise);
  const menu = await appendPetAiUsageMenu([], settings, cache, report);
  expect(texts()[0]).toContain("更新中");
  expect(texts()[1]).toContain("42 tokens");
  menu.update({ ...settings, language: "en-US", providers: [], activeProviderId: "" }); await flush();
  expect(texts()[1]).toBe("Enter an API key to view usage");
  expect(texts()[3]).toBe("Configure AI…");
  response.resolve(local); await flush();
  expect(texts()[1]).toBe("Enter an API key to view usage");
  await menu.dispose();
});

test("partial menu creation leaves all created items with the caller for cleanup", async () => {
  failAt = 2;
  const items: PetMenuItem[] = [];
  await expect(appendPetAiUsageMenu(items, settings, new PetAiUsageCache(async () => local), report)).rejects.toThrow("creation failed");
  expect(items).toHaveLength(2);
  await Promise.all(items.map(item => item.close()));
  expect(closed.size).toBe(2);
});
