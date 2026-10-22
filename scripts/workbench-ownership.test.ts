import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { createSessionOwnershipRouter, sessionRoute, sessionTargetFromUrl } from "../src/workbench/session-route";
import { transformWorkbenchRouting } from "./workbench-routing";

test("native show reclaims the current scope after hidden rejection without DOM visibilitychange", async () => {
  // Given the real generated ownership bridge running against a native host boundary.
  const entry = readFileSync(resolve(import.meta.dir, "workbench-overlay/workbench/entry.tsx"), "utf8");
  const adapted = transformWorkbenchRouting(entry, "E:/yume/src/workbench/session-route.ts");
  const start = adapted.indexOf("  if (bridge) {\n    let routedSessionId");
  const end = adapted.indexOf("  // In-place navigation", start);
  expect(start).toBeGreaterThan(0);
  const code = new Bun.Transpiler({ loader: "ts" }).transformSync(adapted.slice(start, end));
  const document = Object.assign(new EventTarget(), { visibilityState: "visible" });
  let tick = () => {};
  const window = Object.assign(new EventTarget(), {
    setInterval: (callback: () => void) => { tick = callback; return 1; },
    clearInterval: (_id: number) => {},
  });
  let visible = true;
  let owned = false;
  const claimed: unknown[] = [];
  const listeners = new Map<string, () => void>();
  const bridge = {
    core: { invoke: async (command: string, target: unknown) => {
      if (command === "workbench_claim_session") {
        if (!visible) throw new Error("workbench_window_hidden");
        owned = true;
        claimed.push(target);
      }
      if (command === "workbench_release_session") owned = false;
      if (command === "workbench_heartbeat") return owned && visible;
      return undefined;
    } },
    event: { listen: async (name: string, callback: () => void) => {
      listeners.set(name, callback);
      return () => { listeners.delete(name); };
    } },
  };
  const target = { directory: "C:/selected-project", sessionId: "ses_selected" };
  const drain = async () => { for (let index = 0; index < 20; index++) await Promise.resolve(); };
  new Function("bridge", "document", "window", "initialUrl", "sessionTargetFromUrl", "createSessionOwnershipRouter", `let syncWorkbenchOwnership;\n${code}`)(
    bridge, document, window, sessionRoute(target), sessionTargetFromUrl, createSessionOwnershipRouter,
  );
  await drain();
  expect(owned).toBe(true);
  visible = false;
  owned = false;
  tick();
  await drain();
  expect(owned).toBe(false);
  // When the native host shows the unchanged route and sends its shown event.
  visible = true;
  listeners.get("workbench://shown")?.();
  await drain();
  // Then the same composite session is reclaimed and the listener cleans up on unload.
  expect(owned).toBe(true);
  expect(claimed).toEqual([target, target]);
  window.dispatchEvent(new Event("pagehide"));
  await drain();
  expect(listeners.size).toBe(0);
});


test("lazy handoff waits for the actual router and serializes ready/event races", async () => {
  const entry = readFileSync(resolve(import.meta.dir, "workbench-overlay/workbench/entry.tsx"), "utf8");
  const adapted = transformWorkbenchRouting(entry, "E:/yume/src/workbench/session-route.ts");
  const start = adapted.indexOf("  // Listen before taking the saved target.");
  const end = adapted.indexOf("\n}\n\nvoid main()", start);
  const code = new Bun.Transpiler({ loader: "ts" }).transformSync(adapted.slice(start, end));
  let markReady!: () => void;
  const historyReady = new Promise<void>((resolve) => { markReady = resolve; });
  const routes: string[] = [];
  const activeHistory = { set: ({ value }: { value: string }) => { routes.push(value); } };
  const window = new EventTarget();
  let notify!: () => void;
  let pending: { directory: string; sessionId: string } | null = { directory: "C:/one", sessionId: "ses_same" };
  let reads = 0;
  let concurrent = 0;
  let maximumConcurrent = 0;
  let cleaned = false;
  const bridge = {
    event: { listen: async (_name: string, callback: () => void) => { notify = callback; return () => { cleaned = true; }; } },
    core: { invoke: async (command: string) => {
      expect(command).toBe("workbench_take_handoff");
      reads += 1;
      maximumConcurrent = Math.max(maximumConcurrent, ++concurrent);
      await Promise.resolve();
      const target = pending;
      pending = null;
      concurrent -= 1;
      return target;
    } },
  };
  const run = new Function("bridge", "window", "historyReady", "activeHistory", "sessionRoute", `return (async () => { ${code} })()`)(bridge, window, historyReady, activeHistory, sessionRoute);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  expect(reads).toBe(0);
  pending = { directory: "C:/two", sessionId: "ses_same" };
  notify();
  notify();
  markReady();
  await run;
  for (let i = 0; i < 20; i++) await Promise.resolve();
  expect(routes).toEqual([sessionRoute({ directory: "C:/two", sessionId: "ses_same" })]);
  expect(maximumConcurrent).toBe(1);
  pending = { directory: "C:/one", sessionId: "ses_next" };
  notify();
  for (let i = 0; i < 20; i++) await Promise.resolve();
  expect(routes[1]).toBe(sessionRoute({ directory: "C:/one", sessionId: "ses_next" }));
  window.dispatchEvent(new Event("pagehide"));
  expect(cleaned).toBe(true);
});
