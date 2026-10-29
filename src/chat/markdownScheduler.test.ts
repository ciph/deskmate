import { expect, test } from "bun:test";
import { createMarkdownScheduler, MARKDOWN_CACHE_BYTES } from "./markdownScheduler";
import type { MarkdownResult } from "./markdownPipeline";
function fixture() {
  const requests: { id: number; version: number; reset?: string; append?: string; streaming: boolean; at: number }[] = [];
  const ports: ReturnType<typeof port>[] = [];
  function port() { return { onmessage: null as Worker["onmessage"], onerror: null as Worker["onerror"], terminate() {}, postMessage(request: Omit<(typeof requests)[number], "at">) { requests.push({ ...request, at: performance.now() }); } }; }
  const scheduler = createMarkdownScheduler(() => { const worker = port(); ports.push(worker); return worker; });
  function finish(bytes = 100) { const request = requests.at(-1)!; const result: MarkdownResult = { blocks: [], parseMs: 1, bytes }; ports.at(-1)!.onmessage?.call(ports.at(-1) as unknown as Worker, { data: { ...result, id: request.id, version: request.version } } as MessageEvent); }
  return { scheduler, requests, ports, finish };
}
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
