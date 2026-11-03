import { expect, mock, test } from "bun:test";
import { dict } from "../lib/i18n";
import type { AiUsage } from "../lib/settings";
import { kuroProviderFixture } from "../testing/settingsFixtures";
import { PetAiUsageCache, petAiUsageLines } from "./petAiUsage";

const local: AiUsage = { kind: "local", localAvailable: true, todayTokens: 1234, todayRequests: 2, topModels: [] };
function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<T>((done, fail) => { resolve = done; reject = fail; });
  return { resolve, reject, promise };
}
const lines = (usage: AiUsage) => petAiUsageLines({ kind: "ready", usage }, dict("zh-CN"));

test("summaries keep platform cost, currency units and local tokens distinct", () => {
  expect(lines({ remainingCny: 27260000, limitCny: 30000000, remainingPct: 91,
    daysUntilReset: 6, todayCostCny: 2735900, todayRequests: 438, topModels: [] }))
    .toEqual(["本周剩余  91% · ¥2726 / ¥3000", "平台今日费用  ¥273.59 · 438 次"]);
  expect(lines({ ...local, kind: "deepseek", isAvailable: true, balances: [
    { currency: "CNY", totalBalance: "12.34" }, { currency: "USD", totalBalance: "2.00" },
  ] })).toEqual(["账户余额  ¥12.34 · $2.00", "Yume 今日用量  1,234 tokens · 2 次"]);
  expect(lines({ ...local, kind: "openrouter", limitUsd: 10, remainingUsd: 0, todayCostUsd: 1.5 }))
    .toEqual(["API Key 剩余额度  $0.00", "平台今日费用  $1.50"]);
  expect(lines(local)).toEqual(["Yume 今日用量  1,234 tokens · 2 次", "仅统计 Yume 记录到的请求"]);
});

test("unknown values never become zero, and an unlimited key is not a balance", () => {
  expect(lines({ ...local, kind: "openrouter", limitUsd: null, remainingUsd: null, todayCostUsd: 0 })[0])
    .toBe("此 Key 未设置消费上限");
  expect(lines({ ...local, kind: "openrouter", limitUsd: 10, remainingUsd: null, todayCostUsd: 0 })[0])
    .toBe("API Key 剩余额度  —");
  expect(lines({ ...local, localAvailable: false })[0]).toBe("Yume 本地用量暂时无法读取");
  expect(lines({ ...local, kind: "deepseek", isAvailable: false, balances: [] })[0]).toBe("账户余额  —");
});

test("all supported languages have loading, details and updating copy", () => {
  for (const lang of ["zh-CN", "en-US", "ja-JP", "ko-KR"]) {
    const t = dict(lang);
    expect(petAiUsageLines({ kind: "loading" }, t)).toEqual([t.aiUsageLoading, t.petAiUsageSettingsHint]);
    expect(t.petAiUsageDetails.length).toBeGreaterThan(0);
    expect(t.petAiUsageUpdating.length).toBeGreaterThan(0);
  }
});

test("unconfigured providers do not fetch", () => {
  const fetch = mock(async () => local);
  const cache = new PetAiUsageCache(fetch);
  expect(cache.read(null).view.kind).toBe("missing-key");
  expect(cache.read({ ...kuroProviderFixture, apiKey: " " }).refreshed).toBeNull();
  expect(fetch).not.toHaveBeenCalled();
});

test("reads share an in-flight request and reuse a fresh snapshot for 60 seconds", async () => {
  const result = deferred<AiUsage>();
  const fetch = mock(() => result.promise);
  let now = new Date(2026, 10, 2, 12).getTime();
  const cache = new PetAiUsageCache(fetch, () => now);
  const first = cache.read(kuroProviderFixture);
  expect(first.view.kind).toBe("loading");
  expect(cache.read({ ...kuroProviderFixture }).refreshed).toBe(first.refreshed);
  expect(fetch).toHaveBeenCalledTimes(1);
  result.resolve(local); await first.refreshed;
  now += 59_999;
  expect(cache.read(kuroProviderFixture)).toEqual({ view: { kind: "ready", usage: local }, refreshed: null });
  now += 1;
  const stale = cache.read(kuroProviderFixture);
  expect(stale.view).toEqual({ kind: "ready", usage: local });
  expect(stale.refreshed).not.toBeNull();
  await stale.refreshed;
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("midnight and clock rollback invalidate today's cached statistics", async () => {
  let now = new Date(2026, 10, 2, 23, 59, 55).getTime();
  const fetch = mock(async () => local);
  const cache = new PetAiUsageCache(fetch, () => now);
  await cache.read(kuroProviderFixture).refreshed;
  now += 10_000;
  await cache.read(kuroProviderFixture).refreshed;
  now -= 5_000;
  await cache.read(kuroProviderFixture).refreshed;
  expect(fetch).toHaveBeenCalledTimes(3);
});

test("late account responses cannot replace the current account cache", async () => {
  const a = deferred<AiUsage>();
  const b = deferred<AiUsage>();
  const fetch = mock((id: string) => id === kuroProviderFixture.id ? a.promise : b.promise);
  const cache = new PetAiUsageCache(fetch);
  const old = cache.read(kuroProviderFixture);
  const other = { ...kuroProviderFixture, id: "other" };
  const current = cache.read(other);
  b.resolve(local); await current.refreshed;
  a.resolve({ ...local, todayTokens: 999 }); await old.refreshed;
  expect(cache.read(other).view).toEqual({ kind: "ready", usage: local });
  expect(fetch).toHaveBeenCalledTimes(2);
});

test("editing credentials or endpoint invalidates cache, but renaming a provider does not", async () => {
  const fetch = mock(async () => local);
  const cache = new PetAiUsageCache(fetch);
  await cache.read(kuroProviderFixture).refreshed;
  expect(cache.read({ ...kuroProviderFixture, label: "renamed" }).refreshed).toBeNull();
  let provider = { ...kuroProviderFixture };
  for (const change of [{ apiKey: "new-key" }, { baseUrl: "https://example.com/v1" }, { manualModelIds: "model" }]) {
    provider = { ...provider, ...change };
    await cache.read(provider).refreshed;
  }
  expect(fetch).toHaveBeenCalledTimes(4);
});

test("a failed refresh clears stale data and retries on the next open", async () => {
  const response = deferred<AiUsage>();
  let now = 100_000;
  const fetch = mock(() => response.promise);
  const cache = new PetAiUsageCache(fetch, () => now);
  const loading = cache.read(kuroProviderFixture);
  response.resolve(local); await loading.refreshed;
  now += 60_000;
  fetch.mockImplementation(async () => { throw new Error("network"); });
  expect(await cache.read(kuroProviderFixture).refreshed).toEqual({ kind: "unavailable" });
  fetch.mockImplementation(async () => local);
  expect(await cache.read(kuroProviderFixture).refreshed).toEqual({ kind: "ready", usage: local });
});

test("authentication errors use specific localized messages without exposing raw errors", async () => {
  for (const [error, kind] of [["deepseek_auth_failed", "invalid-key"], ["Error: status:403", "unauthorized"], ["secret-value", "unavailable"]] as const) {
    const cache = new PetAiUsageCache(async () => { throw new Error(error); });
    expect(await cache.read(kuroProviderFixture).refreshed).toEqual({ kind });
  }
});
