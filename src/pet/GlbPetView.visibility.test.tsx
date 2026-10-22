import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { act, cleanup, render } from "@testing-library/react";
import { originalTauriWindow } from "../testing/tauriModuleFixture";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { PetRenderer } from "./PetRenderer";
import { GlbPetView } from "./GlbPetView";

const realRenderer = PetRenderer;
const positions = mock(async () => ({ x: 0, y: 0 }));
const scales = mock(async () => 1);
const cursor = mock(async () => ({ x: 20, y: 20 }));
const targets: { readonly x: number; readonly y: number }[] = [];
const visibility: boolean[] = [];
const stopped: string[] = [];
const listeners = new Map<string, () => void>();
const subscribe = (event: string) => async (callback: () => void) => {
  listeners.set(event, callback);
  return () => { stopped.push(event); listeners.delete(event); };
};
class GpuBoundary {
  async load() {}
  setMood() {}
  setRenderTuning() {}
  setMouseFollowEnabled() {}
  setVisible(value: boolean) { visibility.push(value); }
  setMouseTarget(value: { readonly x: number; readonly y: number }) { targets.push(value); }
  dispose() {}
}
const settings = legacySettingsFixture({ personaId: "xiaozhu-nidaime", mouseFollow: true });
const report = () => {};
const rendererRef = { current: null as PetRenderer | null };
const viewProps = { settings, mood: "idle" as const, width: 160, height: 200, rendererRef, onError: report };
beforeEach(() => {
  positions.mockClear(); scales.mockClear(); cursor.mockReset(); cursor.mockImplementation(async () => ({ x: 20, y: 20 }));
  targets.length = 0; visibility.length = 0; stopped.length = 0; listeners.clear();
  mock.module("./PetRenderer", () => ({ PetRenderer: GpuBoundary }));
  mock.module("@tauri-apps/api/window", () => ({ ...originalTauriWindow, cursorPosition: cursor, getCurrentWindow: () => ({
    outerPosition: positions, scaleFactor: scales,
    onMoved: subscribe("move"), onResized: subscribe("resize"), onScaleChanged: subscribe("scale"),
  }) }));
});
afterEach(async () => {
  cleanup(); await act(async () => {});
  mock.module("./PetRenderer", () => ({ PetRenderer: realRenderer }));
  mock.module("@tauri-apps/api/window", () => originalTauriWindow);
});

test("hidden GLB starts no pointer polling and refreshes geometry on each resume", async () => {
  const view = render(<GlbPetView {...viewProps} visible={false} />);
  await act(async () => {});
  expect(positions).toHaveBeenCalledTimes(0);
  expect(cursor).toHaveBeenCalledTimes(0);
  expect(visibility).toEqual([false]);
  view.rerender(<GlbPetView {...viewProps} visible />);
  await act(async () => {});
  expect(positions).toHaveBeenCalledTimes(1);
  expect(scales).toHaveBeenCalledTimes(1);
  expect(cursor).toHaveBeenCalledTimes(1);
  expect(targets).toHaveLength(1);
  view.rerender(<GlbPetView {...viewProps} visible={false} />);
  await act(async () => {});
  expect(stopped.sort()).toEqual(["move", "resize", "scale"]);
  expect(cursor).toHaveBeenCalledTimes(1);
  view.rerender(<GlbPetView {...viewProps} visible />);
  await act(async () => {});
  expect(positions).toHaveBeenCalledTimes(2);
  expect(scales).toHaveBeenCalledTimes(2);
  expect(visibility).toEqual([false, true, false, true]);
});

for (const change of ["hide", "move", "scale", "unmount"]) test(`late GLB cursor is ignored after ${change}`, async () => {
  let finish: ((position: { x: number; y: number }) => void) | undefined;
  cursor.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const view = render(<GlbPetView {...viewProps} visible />);
  await act(async () => {});
  expect(finish).toBeDefined();
  if (change === "hide") view.rerender(<GlbPetView {...viewProps} visible={false} />);
  else if (change === "unmount") view.unmount();
  else listeners.get(change)?.();
  await act(async () => { finish?.({ x: 1000, y: 1000 }); });
  expect(targets).toHaveLength(0);
});
