import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { act, cleanup, render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import type { PetMood } from "../lib/petState";
import { legacySettingsFixture } from "../testing/settingsFixtures";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { PetRenderer } from "./PetRenderer";
import { manualClock } from "../testing/petSleepClock";
import PetApp from "./PetApp";

const originalRenderer = PetRenderer;
class GpuBoundary {
  constructor(private readonly canvas: HTMLCanvasElement) { canvas.dataset.mood = "idle"; }
  async load(id: string) { this.canvas.dataset.persona = id; }
  setMood(mood: PetMood) { this.canvas.dataset.mood = mood; }
  setVisible(visible: boolean) { this.canvas.dataset.visible = String(visible); }
  setRenderTuning() {}
  setMouseFollowEnabled() {}
  dispose() {}
}
const settings = legacySettingsFixture({ personaId: "xiaozhu", mouseFollow: false });
beforeEach(() => {
  restoreTauriModuleFixture(); mockWindows("pet");
  mock.module("./PetRenderer", () => ({ PetRenderer: GpuBoundary }));
  mockIPC(command => {
    if (command === "get_settings") return settings;
    if (command === "get_pet_visibility" || command === "plugin:window|is_visible") return true;
    return null;
  }, { shouldMockEvents: true });
});
afterEach(async () => { cleanup(); await act(async () => {}); clearMocks(); mock.module("./PetRenderer", () => ({ PetRenderer: originalRenderer })); });
for (const change of ["persona", "revision"]) test(`GLB retains true busy mood through ${change} replacement without another mood event`, async () => {
  // Given the real event subscriber and an already loaded GLB renderer.
  const time = manualClock(); const view = render(<PetApp clock={time.clock} />); await act(async () => {});
  await act(async () => { await emit("deskmate://pet-activity", { type: "start", sessionId: "session", requestId: "request", producerEpoch: 1, sequence: 1, eventId: "1:1" }); });
  const before = view.container.querySelector("canvas"); expect(before?.getAttribute("data-mood")).toBe("thinking");
  // When the renderer is replaced, without sending another mood notification.
  await act(async () => {
    if (change === "persona") await emit("deskmate://settings-changed", { ...settings, personaId: "xiaozhu-nidaime" });
    else await emit("deskmate://pack-imported", { packId: "ai-substitute", personaIds: ["xiaozhu"], version: "1.1.0", sha256: "2".repeat(64) });
  });
  // Then the replacement immediately inherits thinking rather than its idle default.
  act(() => time.advance(90000));
  expect(view.container.querySelector(".pet-root")?.getAttribute("data-sleep-state")).toBe("awake");
  const after = view.container.querySelector("canvas"); expect(after === before).toBe(false); expect(after?.getAttribute("data-mood")).toBe("thinking");
});

test("GLB error and ordinary working moods retain their original behavior across role replacement", async () => {
  const time = manualClock();
  const view = render(<PetApp clock={time.clock} />); await act(async () => {});
  const scope = { sessionId: "session", requestId: "failed", producerEpoch: 2 };
  await act(async () => {
    await emit("deskmate://pet-activity", { ...scope, type: "start", sequence: 1, eventId: "2:1" });
    await emit("deskmate://pet-activity", { ...scope, type: "mood", mood: "working", sequence: 2, eventId: "2:2" });
  });
  expect(view.container.querySelector("canvas")?.getAttribute("data-mood")).toBe("working");
  await act(async () => { await emit("deskmate://pet-activity", { ...scope, type: "error", sequence: 3, eventId: "2:3" }); });
  act(() => time.advance(90000));
  expect(view.container.querySelector("canvas")?.getAttribute("data-mood")).toBe("error");
  await act(async () => { await emit("deskmate://settings-changed", { ...settings, personaId: "xiaozhu-nidaime" }); });
  expect(view.container.querySelector("canvas")?.getAttribute("data-mood")).toBe("error");
  await act(async () => {
    await emit("deskmate://pet-activity", { ...scope, requestId: "next", type: "start", sequence: 4, eventId: "2:4" });
    await emit("deskmate://pet-activity", { ...scope, requestId: "next", type: "success", sequence: 5, eventId: "2:5" });
  });
  expect(view.container.querySelector("canvas")?.getAttribute("data-mood")).toBe("idle");
});
