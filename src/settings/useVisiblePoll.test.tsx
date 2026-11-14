import { afterEach, expect, jest, test } from "bun:test";
import { act, cleanup, renderHook } from "@testing-library/react";
import { useVisiblePoll } from "./useVisiblePoll";

afterEach(() => { cleanup(); jest.useRealTimers(); });
const flush = () => act(async () => {});
const tick = (ms: number) => act(async () => { jest.advanceTimersByTime(ms); });
function requests() {
  const calls: { current: () => boolean; done: () => void }[] = [];
  const task = (current: () => boolean) => new Promise<void>(done => calls.push({ current, done }));
  return { calls, task };
}

test("hidden polling is silent, reveal refreshes immediately, slow requests never overlap", async () => {
  jest.useFakeTimers(); const { calls, task } = requests();
  const hook = renderHook(({ visible }) => useVisiblePoll(task, visible, 5000), { initialProps: { visible: false } });
  await tick(600_000); expect(calls).toHaveLength(0);
  hook.rerender({ visible: true }); await flush(); expect(calls).toHaveLength(1);
  await tick(60_000); expect(calls).toHaveLength(1);
  hook.rerender({ visible: false }); expect(calls[0]!.current()).toBe(false);
  hook.rerender({ visible: true }); await flush(); expect(calls).toHaveLength(1);
  await act(async () => { calls[0]!.done(); }); expect(calls).toHaveLength(2);
  expect(calls[1]!.current()).toBe(true);
  await act(async () => { calls[1]!.done(); });
  await tick(4999); expect(calls).toHaveLength(2); await tick(1); expect(calls).toHaveLength(3);
  hook.unmount(); expect(calls[2]!.current()).toBe(false);
  await act(async () => { calls[2]!.done(); }); await tick(60_000); expect(calls).toHaveLength(3);
});

test("provider switches and repeated manual refreshes coalesce behind the current request", async () => {
  jest.useFakeTimers(); const old = requests(), next = requests();
  const hook = renderHook(({ task }) => useVisiblePoll(task, true, 5000), { initialProps: { task: old.task } });
  await flush(); hook.rerender({ task: next.task });
  act(() => { hook.result.current(); hook.result.current(); }); await flush();
  expect(old.calls[0]!.current()).toBe(false); expect(next.calls).toHaveLength(0);
  await act(async () => { old.calls[0]!.done(); }); expect(next.calls).toHaveLength(1);
  await act(async () => { next.calls[0]!.done(); }); await tick(5000); expect(next.calls).toHaveLength(2);
});

test("stagger applies only to first visible activation and hiding cancels deferred work", async () => {
  jest.useFakeTimers(); const { calls, task } = requests();
  const hook = renderHook(({ visible }) => useVisiblePoll(task, visible, 300_000, 3000), { initialProps: { visible: true } });
  await tick(2999); expect(calls).toHaveLength(0); hook.rerender({ visible: false });
  await tick(600_000); expect(calls).toHaveLength(0);
  hook.rerender({ visible: true }); await flush(); expect(calls).toHaveLength(1);
  hook.rerender({ visible: false }); await act(async () => { calls[0]!.done(); });
  await tick(600_000); expect(calls).toHaveLength(1);
});

test("explicit refresh cancels the initial stagger instead of issuing a second request", async () => {
  jest.useFakeTimers(); const { calls, task } = requests();
  const hook = renderHook(() => useVisiblePoll(task, true, 300_000, 3000));
  act(() => hook.result.current()); await flush(); expect(calls).toHaveLength(1);
  await act(async () => { calls[0]!.done(); }); await tick(3000);
  expect(calls).toHaveLength(1);
});
