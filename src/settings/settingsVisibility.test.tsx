import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import * as core from "@tauri-apps/api/core";
import * as events from "@tauri-apps/api/event";
import { act, cleanup, renderHook } from "@testing-library/react";

let native = true;
let listener: ((event: { payload: boolean }) => void) | undefined;
const off = mock(() => {});
const queries: ((value: boolean) => void)[] = [];
const invoke = mock(() => new Promise<boolean>(resolve => queries.push(resolve)));
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke, isTauri: () => native }));
mock.module("@tauri-apps/api/event", () => ({ ...events, listen: (_: string, callback: typeof listener) => { listener = callback; return Promise.resolve(off); } }));
const { useSettingsWindowVisible } = await import("./settingsVisibility");
beforeEach(() => { native = true; queries.length = 0; invoke.mockClear(); off.mockClear(); Object.defineProperty(document, "hidden", { configurable: true, value: false }); });
afterEach(() => { cleanup(); Object.defineProperty(document, "hidden", { configurable: true, value: false }); });

test("native hide overrides a late visible probe and reveal refreshes visibility", async () => {
  const hook = renderHook(useSettingsWindowVisible);
  await act(async () => {}); expect(hook.result.current).toBe(false);
  await act(async () => { listener?.({ payload: false }); queries[0]!(true); });
  expect(hook.result.current).toBe(false);
  act(() => listener?.({ payload: true })); expect(hook.result.current).toBe(true);
  Object.defineProperty(document, "hidden", { configurable: true, value: true });
  await act(async () => { document.dispatchEvent(new Event("visibilitychange")); queries[1]!(true); });
  expect(hook.result.current).toBe(false);
  Object.defineProperty(document, "hidden", { configurable: true, value: false });
  await act(async () => { window.dispatchEvent(new Event("focus")); queries[2]!(true); });
  expect(hook.result.current).toBe(true);
  hook.unmount(); await act(async () => {});
  expect(off).toHaveBeenCalledTimes(1); window.dispatchEvent(new Event("focus")); expect(queries).toHaveLength(3);
});

test("browser fallback respects document visibility and early unmount never probes native", async () => {
  const early = renderHook(useSettingsWindowVisible); early.unmount(); await act(async () => {});
  expect(invoke).not.toHaveBeenCalled();
  native = false; const browser = renderHook(useSettingsWindowVisible); await act(async () => {});
  expect(browser.result.current).toBe(true);
  Object.defineProperty(document, "hidden", { configurable: true, value: true });
  act(() => document.dispatchEvent(new Event("visibilitychange"))); expect(browser.result.current).toBe(false);
});
