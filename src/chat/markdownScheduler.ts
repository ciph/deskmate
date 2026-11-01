import type { MarkdownBlock, MarkdownResult } from "./markdownPipeline";

export const MARKDOWN_INTERVAL_MS = 125;
export const MARKDOWN_CACHE_BYTES = 16 * 1024 * 1024;
export interface MarkdownSnapshot { text: string; streaming: boolean; blocks: readonly MarkdownBlock[]; unavailable?: boolean }
interface Job { id: number; version: number; text: string; streaming: boolean; terminal?: boolean; publish: (snapshot: MarkdownSnapshot) => void }
interface Client { job: Job; lastStart: number; previous?: MarkdownSnapshot }
type WorkerPort = Pick<Worker, "postMessage" | "terminate" | "onmessage" | "onerror">;

/** One window, one worker, one running request and at most one pending per view. */
export function createMarkdownScheduler(factory: () => WorkerPort) {
  let worker: WorkerPort | null = null, failures = 0, sequence = 0;
  let running: Job | null = null, workerSource: Job | null = null;
  let timer: ReturnType<typeof setTimeout> | undefined;
  const clients = new Map<number, Client>();
  const pending = new Map<number, Job>();
  const cache = new Map<string, { snapshot: MarkdownSnapshot; bytes: number }>();
  let cacheBytes = 0;
  let observe: ((event: { kind: "start" | "finish"; id: number; version: number; at: number; chars: number; streaming: boolean; terminal: boolean; parseMs?: number }) => void) | undefined;
  const stats = { starts: 0, maxRunning: 0, maxPendingPerMessage: 0, parseMs: 0, failures: 0, cacheBytes: 0 };
  const keyOf = (job: Job) => `${job.streaming ? "stream:" : "final:"}${job.text}`;
  function publish(job: Job, snapshot: MarkdownSnapshot) {
    const client = clients.get(job.id);
    if (!client || client.job.version !== job.version) return;
    client.previous = snapshot;
    job.publish(snapshot);
  }
  function failed() {
    worker?.terminate(); worker = null; workerSource = null;
    failures += 1; stats.failures = failures;
    if (running && !pending.has(running.id) && clients.has(running.id)) pending.set(running.id, running);
    running = null;
    for (const client of clients.values()) client.lastStart = -Infinity;
    if (failures > 1) {
      for (const job of pending.values()) publish(job, { text: job.text, streaming: job.streaming, blocks: [], unavailable: true });
      pending.clear();
    } else pump();
  }
  function startWorker() {
    if (worker) return true;
    try {
      worker = factory();
      worker.onerror = () => failed();
      worker.onmessage = event => {
        const result = event.data as MarkdownResult & { id: number; version: number };
        const job = running;
        if (!job || result.id !== job.id || result.version !== job.version) return;
        running = null;
        stats.parseMs += result.parseMs;
        observe?.({ kind: "finish", id: job.id, version: job.version, at: performance.now(), chars: job.text.length, streaming: job.streaming, terminal: !!job.terminal, parseMs: result.parseMs });
        const previous = clients.get(job.id)?.previous;
        const oldBlocks = new Map(previous?.blocks.map(block => [block.key, block]));
        const blocks = result.blocks.map(block => oldBlocks.get(block.key)?.fingerprint === block.fingerprint ? oldBlocks.get(block.key)! : block);
        const snapshot = { text: job.text, streaming: job.streaming, blocks };
        const key = keyOf(job);
        if (result.bytes <= MARKDOWN_CACHE_BYTES) {
          cacheBytes -= cache.get(key)?.bytes ?? 0;
          cache.delete(key); cache.set(key, { snapshot, bytes: result.bytes }); cacheBytes += result.bytes;
          while (cacheBytes > MARKDOWN_CACHE_BYTES) {
            const first = cache.entries().next().value;
            if (!first) break;
            cacheBytes -= first[1].bytes; cache.delete(first[0]);
          }
          stats.cacheBytes = cacheBytes;
        }
        // A newer prefix may still use this formatted portion plus a raw tail.
        const client = clients.get(job.id);
        if (client && client.job.text.startsWith(job.text) && (client.job.streaming || client.job.version === job.version)) {
          client.previous = snapshot; client.job.publish(snapshot);
        }
        pump();
      };
      return true;
    } catch { failed(); return false; }
  }
  function pump() {
    clearTimeout(timer); timer = undefined;
    if (running || !pending.size || failures > 1) return;
    const now = performance.now();
    let delay = Infinity;
    const priority = (job: Job) => Number(!!job.terminal) * 2 + Number(job.streaming);
    const jobs = [...pending.values()].sort((a, b) => priority(b) - priority(a));
    for (const job of jobs) {
      const client = clients.get(job.id);
      if (!client) { pending.delete(job.id); continue; }
      const wait = job.streaming ? Math.max(0, client.lastStart + MARKDOWN_INTERVAL_MS - now) : 0;
      if (wait > 0) { delay = Math.min(delay, wait); continue; }
      const cached = cache.get(keyOf(job));
      if (cached) {
        cache.delete(keyOf(job)); cache.set(keyOf(job), cached);
        pending.delete(job.id); publish(job, cached.snapshot); continue;
      }
      if (!startWorker()) return;
      pending.delete(job.id); running = job; client.lastStart = now;
      stats.starts += 1; stats.maxRunning = 1;
      observe?.({ kind: "start", id: job.id, version: job.version, at: now, chars: job.text.length, streaming: job.streaming, terminal: !!job.terminal });
      const append = workerSource?.id === job.id && job.text.startsWith(workerSource.text);
      worker!.postMessage({ id: job.id, version: job.version, streaming: job.streaming,
        ...(append ? { append: job.text.slice(workerSource!.text.length) } : { reset: job.text }) });
      workerSource = job;
      return;
    }
    if (pending.size && Number.isFinite(delay)) timer = setTimeout(pump, delay);
  }
  return {
    stats,
    observe(listener: typeof observe) { observe = listener; return () => { if (observe === listener) observe = undefined; }; },
    open(publishResult: Job["publish"]) {
      const id = ++sequence;
      const client: Client = { lastStart: -Infinity, job: { id, version: 0, text: "", streaming: false, publish: publishResult } };
      clients.set(id, client);
      return {
        update(text: string, streaming: boolean) {
          client.job = { ...client.job, version: client.job.version + 1, text, streaming, terminal: client.job.streaming && !streaming };
          if (failures > 1) { publish(client.job, { text, streaming, blocks: [], unavailable: true }); return; }
          pending.set(id, client.job); stats.maxPendingPerMessage = 1; pump();
        },
        close() { clients.delete(id); pending.delete(id); },
      };
    },
    clearCache() { cache.clear(); cacheBytes = 0; stats.cacheBytes = 0; },
    dispose() { clearTimeout(timer); worker?.terminate(); worker = null; clients.clear(); pending.clear(); cache.clear(); cacheBytes = 0; running = null; },
  };
}

export const markdownScheduler = createMarkdownScheduler(() => new Worker(new URL("./markdown.worker.ts", import.meta.url), { type: "module" }));
