import { afterEach, beforeEach, expect, mock, spyOn, test } from "bun:test";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useGifPassthrough, createCursorWriter } from "./useGifPassthrough";
import { originalTauriWindow } from "../testing/tauriModuleFixture";

const position = { x: 500, y: 500 };
const origin = mock(async () => ({ x: 0, y: 0 }));
const ratio = mock(async () => 1);
const writes: boolean[] = [];
let finishWrite: (() => void) | undefined;
let finishCursor: ((value: typeof position) => void) | undefined;
let pendingCursor = false;
let holdWrite = true;
const cursor = mock(() => pendingCursor ? new Promise<typeof position>((resolve) => { finishCursor = resolve; }) : Promise.resolve(position));
const events: Partial<Record<"move" | "resize" | "scale", () => void>> = {};
const stopped = mock(() => {});
const onMoved = mock(async (callback: () => void) => { events.move = callback; return stopped; });
const onResized = mock(async (callback: () => void) => { events.resize = callback; return stopped; });
const onScaleChanged = mock(async (callback: () => void) => { events.scale = callback; return stopped; });
const timers = new Map<number, () => void>();
let timerId = 0;
let restoreTimers: () => void;

beforeEach(() => {
  writes.length = 0; finishWrite = undefined; finishCursor = undefined; pendingCursor = false; holdWrite = true;
  origin.mockReset().mockResolvedValue({ x: 0, y: 0 }); ratio.mockReset().mockResolvedValue(1); cursor.mockClear();
  stopped.mockClear(); onMoved.mockClear(); onResized.mockClear(); onScaleChanged.mockClear();
  delete events.move; delete events.resize; delete events.scale;
  timers.clear(); timerId = 0;
  const interval = spyOn(window, "setInterval").mockImplementation(((callback: () => void, delay: number) => {
    expect(delay).toBe(40);
    const id = ++timerId; timers.set(id, callback); return id;
  }) as typeof window.setInterval);
  const clear = spyOn(window, "clearInterval").mockImplementation((id) => { timers.delete(Number(id)); });
  restoreTimers = () => { interval.mockRestore(); clear.mockRestore(); };
  mock.module("@tauri-apps/api/window", () => ({ ...originalTauriWindow, cursorPosition: cursor, getCurrentWindow: () => ({
    outerPosition: origin, scaleFactor: ratio, onMoved, onResized, onScaleChanged,
    setIgnoreCursorEvents: (value: boolean) => { writes.push(value); return holdWrite ? new Promise<void>((resolve) => { finishWrite = resolve; }) : Promise.resolve(); },
  }) }));
});
afterEach(() => { cleanup(); finishWrite?.(); restoreTimers(); mock.module("@tauri-apps/api/window", () => originalTauriWindow); });

async function tick() {
  await act(async () => { for (const callback of [...timers.values()]) callback(); });
}
function petRoot() {
  const root = document.createElement("div");
  const image = document.createElement("img"); image.className = "gif-pet-image";
  image.getBoundingClientRect = () => new DOMRect(0, 0, 100, 100);
  root.append(image);
  return { current: root };
}

test("a lock invalidates pending cursor lookup before later native reads", async () => {
  pendingCursor = true;
  const root = { current: document.createElement("div") };
  const hook = renderHook(() => useGifPassthrough(true, root));
  act(() => hook.result.current(true));
  await act(async () => { finishCursor?.(position); });
  expect(origin).toHaveBeenCalledTimes(0);
  expect(writes).toEqual([]);
});
test("disable during an in-flight true write restores false after that write completes", async () => {
  const root = { current: document.createElement("div") };
  const hook = renderHook(({ enabled }) => useGifPassthrough(enabled, root), { initialProps: { enabled: true } });
  await act(async () => {});
  expect(writes).toEqual([true]);
  hook.rerender({ enabled: false });
  await act(async () => { finishWrite?.(); });
  expect(writes).toEqual([true, false]);
});
test("unmount during an in-flight true write restores false after completion", async () => {
  const root = { current: document.createElement("div") };
  const hook = renderHook(() => useGifPassthrough(true, root));
  await act(async () => {});
  hook.unmount();
  await act(async () => { finishWrite?.(); });
  expect(writes).toEqual([true, false]);
});
test("a failed native write can be retried by a later poll", async () => {
  const calls: boolean[] = [];
  const setIgnored = createCursorWriter(async (value) => { calls.push(value); if (calls.length === 1) throw new Error("temporary unavailable"); });
  setIgnored(true);
  await Promise.resolve(); await Promise.resolve();
  setIgnored(true);
  await Promise.resolve();
  expect(calls).toEqual([true,true]);
});

test("25 stationary polls read cursor 25 times and position/DPI once; the next 25 need only cursor IPC", async () => {
  holdWrite = false;
  const root = petRoot();
  renderHook(() => useGifPassthrough(true, root));
  await act(async () => {});
  for (let index = 0; index < 24; index += 1) await tick();
  expect(cursor).toHaveBeenCalledTimes(25);
  expect(origin).toHaveBeenCalledTimes(1);
  expect(ratio).toHaveBeenCalledTimes(1);
  cursor.mockClear(); origin.mockClear(); ratio.mockClear();
  for (let index = 0; index < 25; index += 1) await tick();
  expect(cursor).toHaveBeenCalledTimes(25);
  expect(origin).toHaveBeenCalledTimes(0);
  expect(ratio).toHaveBeenCalledTimes(0);
});

test("hidden polling stops entirely and resume obtains fresh position and DPI", async () => {
  holdWrite = false;
  const root = petRoot();
  const hook = renderHook(({ enabled }) => useGifPassthrough(enabled, root), { initialProps: { enabled: true } });
  await act(async () => {});
  expect(writes).toEqual([true]);
  hook.rerender({ enabled: false });
  await act(async () => {});
  expect(timers.size).toBe(0);
  expect(stopped).toHaveBeenCalledTimes(3);
  const cursorBeforeHide = cursor.mock.calls.length;
  for (let index = 0; index < 25; index += 1) await tick();
  expect(cursor).toHaveBeenCalledTimes(cursorBeforeHide);
  origin.mockResolvedValue({ x: 400, y: 400 }); ratio.mockResolvedValue(2);
  hook.rerender({ enabled: true });
  await act(async () => {});
  expect(timers.size).toBe(1);
  expect(cursor).toHaveBeenCalledTimes(cursorBeforeHide + 1);
  expect(origin).toHaveBeenCalledTimes(2);
  expect(ratio).toHaveBeenCalledTimes(2);
  expect(writes).toEqual([true, false]);
});

test("polling is single-flight while cursor lookup is pending", async () => {
  pendingCursor = true;
  const root = petRoot();
  renderHook(() => useGifPassthrough(true, root));
  for (let index = 0; index < 25; index += 1) await tick();
  expect(cursor).toHaveBeenCalledTimes(1);
  expect(origin).toHaveBeenCalledTimes(0);
  await act(async () => { finishCursor?.(position); });
  expect(origin).toHaveBeenCalledTimes(1);
  pendingCursor = false;
  await tick();
  expect(cursor).toHaveBeenCalledTimes(2);
});

test("unmount discards a pending cursor before any geometry or ignore write", async () => {
  pendingCursor = true;
  const root = petRoot();
  const hook = renderHook(() => useGifPassthrough(true, root));
  hook.unmount();
  await act(async () => { finishCursor?.(position); });
  expect(origin).toHaveBeenCalledTimes(0);
  expect(ratio).toHaveBeenCalledTimes(0);
  expect(writes).toEqual([]);
  expect(timers.size).toBe(0);
});

test.each(["lock", "unmount"] as const)("%s discards a pending geometry response", async (action) => {
  let finishOrigin!: (value: { x: number; y: number }) => void;
  origin.mockImplementationOnce(() => new Promise((resolve) => { finishOrigin = resolve; }));
  const root = petRoot();
  const hook = renderHook(() => useGifPassthrough(true, root));
  await act(async () => {});
  expect(origin).toHaveBeenCalledTimes(1);
  if (action === "lock") act(() => hook.result.current(true)); else hook.unmount();
  await act(async () => { finishOrigin({ x: 0, y: 0 }); });
  expect(writes).toEqual([]);
});

test.each(["move", "resize", "scale"] as const)("%s during cursor lookup discards that stale sample", async (event) => {
  pendingCursor = true;
  const root = petRoot();
  renderHook(() => useGifPassthrough(true, root));
  events[event]!();
  await act(async () => { finishCursor?.(position); });
  expect(origin).toHaveBeenCalledTimes(0);
  expect(writes).toEqual([]);
  pendingCursor = false;
  await tick();
  expect(origin).toHaveBeenCalledTimes(1);
  expect(writes).toEqual([true]);
});

test("a move during geometry lookup discards old coordinates and retries with the new origin", async () => {
  holdWrite = false;
  let finishOrigin!: (value: { x: number; y: number }) => void;
  origin.mockImplementationOnce(() => new Promise((resolve) => { finishOrigin = resolve; }));
  const root = petRoot();
  renderHook(() => useGifPassthrough(true, root));
  await act(async () => {});
  events.move!();
  origin.mockResolvedValue({ x: 400, y: 400 }); ratio.mockResolvedValue(2);
  await act(async () => { finishOrigin({ x: 0, y: 0 }); });
  expect(writes).toEqual([]);
  await tick();
  expect(origin).toHaveBeenCalledTimes(2);
  expect(ratio).toHaveBeenCalledTimes(2);
  expect(writes).toEqual([]);
});
