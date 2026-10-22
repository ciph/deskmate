import { expect, mock, test } from "bun:test";
import { PhysicalPosition, PhysicalSize } from "@tauri-apps/api/dpi";
import type { EventCallback } from "@tauri-apps/api/event";
import { createPetWindowGeometry } from "./petWindowGeometry";

function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((finish) => { resolve = finish; });
  return { promise, resolve };
}

function windowFixture(waitForSubscriptions = false) {
  const callbacks: (() => void)[] = [];
  const stops = [mock(() => {}), mock(() => {}), mock(() => {})];
  const subscriptions = stops.map(() => deferred<() => void>());
  const listen = <T>(index: number, payload: T) => mock(async (callback: EventCallback<T>) => {
    callbacks[index] = () => callback({ event: "test", id: 0, payload });
    return waitForSubscriptions ? subscriptions[index]!.promise : stops[index]!;
  });
  const window = {
    outerPosition: mock(async () => new PhysicalPosition(100, 200)),
    scaleFactor: mock(async () => 1.5),
    onMoved: listen(0, new PhysicalPosition(0, 0)), onResized: listen(1, new PhysicalSize(100, 100)), onScaleChanged: listen(2, { scaleFactor: 1, size: new PhysicalSize(100, 100) }),
  };
  const report = mock(() => {});
  const geometry = createPetWindowGeometry(window, report);
  return { window, geometry, report, callbacks, stops, ready: () => subscriptions.forEach((subscription, index) => subscription.resolve(stops[index]!)) };
}

async function flush() {
  for (let index = 0; index < 8; index += 1) await Promise.resolve();
}

test("geometry waits for every event subscription and shares one native read", async () => {
  const fixture = windowFixture(true);
  const first = fixture.geometry.read();
  const concurrent = fixture.geometry.read();
  expect(concurrent).toBe(first);
  await flush();
  expect(fixture.window.outerPosition).toHaveBeenCalledTimes(0);
  expect(fixture.window.scaleFactor).toHaveBeenCalledTimes(0);
  fixture.ready();
  expect(await first).toEqual({ x: 100, y: 200, ratio: 1.5 });
  expect(await concurrent).toEqual({ x: 100, y: 200, ratio: 1.5 });
  expect(await fixture.geometry.read()).toEqual({ x: 100, y: 200, ratio: 1.5 });
  expect(fixture.window.outerPosition).toHaveBeenCalledTimes(1);
  expect(fixture.window.scaleFactor).toHaveBeenCalledTimes(1);
  fixture.geometry.dispose();
});

test.each([0, 1, 2])("event %i invalidates a completed position and DPI cache", async (eventIndex) => {
  const fixture = windowFixture();
  await fixture.geometry.read();
  fixture.window.outerPosition.mockResolvedValue(new PhysicalPosition(300, 400));
  fixture.window.scaleFactor.mockResolvedValue(2);
  fixture.callbacks[eventIndex]!();
  expect(fixture.geometry.generation).toBe(1);
  expect(await fixture.geometry.read()).toEqual({ x: 300, y: 400, ratio: 2 });
  expect(fixture.window.outerPosition).toHaveBeenCalledTimes(2);
  expect(fixture.window.scaleFactor).toHaveBeenCalledTimes(2);
  fixture.geometry.dispose();
});

test.each([0, 1, 2])("event %i discards position and DPI that finish after invalidation", async (eventIndex) => {
  const fixture = windowFixture();
  const oldPosition = deferred<PhysicalPosition>();
  const oldRatio = deferred<number>();
  fixture.window.outerPosition.mockImplementationOnce(() => oldPosition.promise);
  fixture.window.scaleFactor.mockImplementationOnce(() => oldRatio.promise);
  const stale = fixture.geometry.read();
  await flush();
  fixture.callbacks[eventIndex]!();
  oldPosition.resolve(new PhysicalPosition(-1000, -1000));
  oldRatio.resolve(4);
  expect(await stale).toBeNull();
  expect(await fixture.geometry.read()).toEqual({ x: 100, y: 200, ratio: 1.5 });
  expect(fixture.window.outerPosition).toHaveBeenCalledTimes(2);
  expect(fixture.window.scaleFactor).toHaveBeenCalledTimes(2);
  fixture.geometry.dispose();
});

test("dispose before subscriptions resolve cleans each late listener without native reads", async () => {
  const fixture = windowFixture(true);
  const pending = fixture.geometry.read();
  fixture.geometry.dispose();
  fixture.ready();
  expect(await pending).toBeNull();
  await flush();
  expect(fixture.stops.map((stop) => stop.mock.calls.length)).toEqual([1, 1, 1]);
  expect(await fixture.geometry.read()).toBeNull();
  expect(fixture.window.outerPosition).toHaveBeenCalledTimes(0);
  expect(fixture.window.scaleFactor).toHaveBeenCalledTimes(0);
  expect(fixture.report).toHaveBeenCalledTimes(0);
});

test("dispose rejects a late native geometry result", async () => {
  const fixture = windowFixture();
  const position = deferred<PhysicalPosition>();
  fixture.window.outerPosition.mockImplementationOnce(() => position.promise);
  const pending = fixture.geometry.read();
  await flush();
  fixture.geometry.dispose();
  position.resolve(new PhysicalPosition(100, 200));
  expect(await pending).toBeNull();
  expect(await fixture.geometry.read()).toBeNull();
  expect(fixture.stops.map((stop) => stop.mock.calls.length)).toEqual([1, 1, 1]);
});
