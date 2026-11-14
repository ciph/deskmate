import { afterEach, expect, jest, test } from "bun:test";
import { createMarkdownScheduler, MARKDOWN_CACHE_BYTES, MARKDOWN_IDLE_MS } from "./markdownScheduler";
import type { MarkdownResult } from "./markdownPipeline";
function fixture() {
  const requests: { id: number; version: number; reset?: string; append?: string; streaming: boolean; at: number }[] = [];
  const ports: ReturnType<typeof port>[] = [];
  function port() { return { onmessage: null as Worker["onmessage"], onerror: null as Worker["onerror"], terminated: false, terminate() { this.terminated = true; }, postMessage(request: Omit<(typeof requests)[number], "at">) { requests.push({ ...request, at: performance.now() }); } }; }
  const scheduler = createMarkdownScheduler(() => { const worker = port(); ports.push(worker); return worker; });
  function finish(bytes = 100, parseMs = 1) { const request = requests.at(-1)!; const result: MarkdownResult = { blocks: [], parseMs, bytes }; ports.at(-1)!.onmessage?.call(ports.at(-1) as unknown as Worker, { data: { ...result, id: request.id, version: request.version } } as MessageEvent); }
  return { scheduler, requests, ports, finish };
}
afterEach(() => jest.useRealTimers());

test("last view closing releases worker and cache after grace, reopening cancels release", () => {
  jest.useFakeTimers(); const f = fixture();
  const first = f.scheduler.open(() => {}); first.update("cached", false); f.finish(100);
  const second = f.scheduler.open(() => {}); first.close();
  jest.advanceTimersByTime(MARKDOWN_IDLE_MS * 2); expect(f.ports[0]!.terminated).toBe(false);
  second.close(); jest.advanceTimersByTime(MARKDOWN_IDLE_MS - 1);
  const reopened = f.scheduler.open(() => {}); reopened.update("cached", false);
  jest.advanceTimersByTime(MARKDOWN_IDLE_MS); expect(f.ports).toHaveLength(1);
  expect(f.ports[0]!.terminated).toBe(false); expect(f.scheduler.stats.cacheBytes).toBe(100);
  reopened.close(); jest.advanceTimersByTime(MARKDOWN_IDLE_MS);
  expect(f.ports[0]!.terminated).toBe(true); expect(f.scheduler.stats.cacheBytes).toBe(0);
  f.scheduler.open(() => {}).update("cached", false);
  expect(f.ports).toHaveLength(2); expect(f.requests.at(-1)!.reset).toBe("cached");
  f.scheduler.dispose();
});

test("idle cancels an orphan parse and stale worker callbacks cannot affect a new worker", () => {
  jest.useFakeTimers(); const f = fixture(); const shown: string[] = [];
  const old = f.scheduler.open(s => shown.push(s.text)); old.update("old", false);
  const message = f.ports[0]!.onmessage!, error = f.ports[0]!.onerror!;
  const oldRequest = f.requests[0]!; old.close(); old.update("closed update", false);
  jest.advanceTimersByTime(MARKDOWN_IDLE_MS);
  expect(f.ports[0]!.terminated).toBe(true); expect(f.requests).toHaveLength(1);
  f.scheduler.open(s => shown.push(s.text)).update("new", false);
  message.call(f.ports[0] as unknown as Worker, { data: { ...oldRequest, blocks: [], bytes: 100, parseMs: 1 } } as MessageEvent);
  error.call(f.ports[0] as unknown as AbstractWorker, {} as ErrorEvent);
  expect(f.scheduler.stats.failures).toBe(0); expect(f.scheduler.stats.cacheBytes).toBe(0);
  f.finish(); expect(shown).toEqual(["new"]); f.scheduler.dispose();
});

test("a new session after idle can recover from the previous session's two worker failures", () => {
  jest.useFakeTimers(); const f = fixture(); const client = f.scheduler.open(() => {});
  client.update("source", false);
  for (let i = 0; i < 2; i++) f.ports[i]!.onerror?.call(f.ports[i] as unknown as AbstractWorker, {} as ErrorEvent);
  client.close(); jest.advanceTimersByTime(MARKDOWN_IDLE_MS);
  let result = ""; f.scheduler.open(s => { result = s.text; }).update("restored", false); f.finish();
  expect(f.ports).toHaveLength(3); expect(result).toBe("restored"); f.scheduler.dispose();
});
test("500 updates coalesce, append transport and final parsing bypass the cadence", async () => {
  const f = fixture(); const shown: string[] = [];
  const client = f.scheduler.open(snapshot => shown.push(snapshot.text));
  client.update("first", true);
  for (let i = 0; i < 500; i++) client.update(`first ${i}`, true);
  expect(f.requests.length).toBe(1);
  f.finish();
  expect(shown).toEqual(["first"]);
  await Bun.sleep(140);
  expect(f.requests.length).toBe(2);
  expect(f.requests[1]!.append).toBe(" 499");
  expect(f.requests[1]!.at - f.requests[0]!.at).toBeGreaterThanOrEqual(124);
  client.update("corrected final", false); f.finish();
  expect(f.requests.at(-1)!.reset).toBe("corrected final"); f.finish();
  expect(shown.at(-1)).toBe("corrected final");
  expect(f.scheduler.stats.maxRunning).toBe(1); expect(f.scheduler.stats.maxPendingPerMessage).toBe(1);
  f.scheduler.dispose();
});
test("closed clients and non-prefix replacements reject stale results", () => {
  const f = fixture(); const shown: string[] = [];
  const client = f.scheduler.open(snapshot => shown.push(snapshot.text));
  client.update("old body", true); client.update("replacement", false); f.finish();
  expect(shown).toEqual([]); client.close(); f.finish(); expect(shown).toEqual([]);
  f.scheduler.dispose();
});
test("worker failure retries once, then exposes copyable original text", () => {
  const f = fixture(); let unavailable = false;
  f.scheduler.open(snapshot => { unavailable = snapshot.unavailable ?? false; }).update("source", true);
  f.ports[0]!.onerror?.call(f.ports[0] as unknown as AbstractWorker, {} as ErrorEvent);
  expect(f.ports.length).toBe(2);
  f.ports[1]!.onerror?.call(f.ports[1] as unknown as AbstractWorker, {} as ErrorEvent);
  expect(unavailable).toBe(true); expect(f.scheduler.stats.failures).toBe(2); f.scheduler.dispose();
});
test("cache is bounded and oversized documents are not retained", () => {
  const f = fixture(); const client = f.scheduler.open(() => {});
  client.update("one", false); f.finish(MARKDOWN_CACHE_BYTES - 1);
  client.update("two", false); f.finish(100);
  expect(f.scheduler.stats.cacheBytes).toBe(100);
  client.update("large", false); f.finish(MARKDOWN_CACHE_BYTES + 1);
  expect(f.scheduler.stats.cacheBytes).toBe(100);
  client.close(); f.scheduler.clearCache(); expect(f.scheduler.stats.cacheBytes).toBe(0); f.scheduler.dispose();
});
test("expensive streaming parses back off, discard prefix cache and let the final update bypass the delay", async () => {
  const f = fixture(); const shown: string[] = [];
  const client = f.scheduler.open(snapshot => shown.push(snapshot.text));
  client.update("first", true); client.update("first newest", true);
  f.finish(100, 500);
  expect(f.scheduler.stats.cacheBytes).toBe(0);
  await Bun.sleep(160);
  expect(f.requests.length).toBe(1);
  client.update("final exact", false);
  expect(f.requests.length).toBe(2);
  expect(f.requests[1]!.reset).toBe("final exact");
  f.finish();
  expect(shown.at(-1)).toBe("final exact");
  f.scheduler.dispose();
});
