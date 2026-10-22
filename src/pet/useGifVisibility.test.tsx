import { afterEach, beforeEach, expect, test } from "bun:test";
import { act, cleanup, render } from "@testing-library/react";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC, mockWindows } from "@tauri-apps/api/mocks";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { useGifVisibility, type VisibilityClock } from "./useGifVisibility";
import { useGifState } from "./useGifState";

beforeEach(() => { restoreTauriModuleFixture(); mockWindows("pet"); });
afterEach(async () => { cleanup(); await act(async () => {}); clearMocks(); });
function Harness({ personaId = "a", gif = true, clock }: { readonly personaId?: string; readonly gif?: boolean; readonly clock?: VisibilityClock }) {
  const visibility = useGifVisibility(personaId, gif, 910, clock);
  const state = useGifState(visibility.visible);
  return <output data-leaving={visibility.leaving} data-visible={visibility.visible} data-native-visible={visibility.nativeVisible}>{state}</output>;
}

test("startup-hidden completion is suppressed when shown", async () => {
  mockIPC((command) => command === "get_pet_visibility" ? false : undefined, { shouldMockEvents: true });
  const view = render(<Harness />);
  await act(async () => {});
  await act(async () => {
    await emit("deskmate://pet-activity", { type: "start", sessionId: "s", requestId: "r", producerEpoch: 1, sequence: 1, eventId: "1:1" });
    await emit("deskmate://pet-activity", { type: "success", sessionId: "s", requestId: "r", producerEpoch: 1, sequence: 2, eventId: "1:2" });
  });
  await act(async () => { await emit("deskmate://pet-visibility", { phase: "reset", token: 2 }); });
  expect(view.container.querySelector("output")?.textContent).toBe("idle");
  expect(view.container.querySelector("output")?.dataset.visible).toBe("true");
});

test("returning to a departed persona clears the old departure before paint", async () => {
  mockIPC((command) => command === "get_pet_visibility" ? true : undefined, { shouldMockEvents: true });
  const view = render(<Harness />);
  await act(async () => {});
  await act(async () => { await emit("deskmate://pet-visibility", { phase: "leaving", token: 1 }); });
  expect(view.container.querySelector("output")?.dataset.leaving).toBe("true");
  view.rerender(<Harness personaId="b" gif={false} />);
  view.rerender(<Harness personaId="a" />);
  expect(view.container.querySelector("output")?.dataset.leaving).toBe("false");
  await act(async () => {});
  expect(view.container.querySelector("output")?.dataset.visible).toBe("true");
});

test("reset cancels the previous 910ms acknowledgement", async () => {
  const acknowledgements: unknown[] = [];
  mockIPC((command, args) => {
    if (command === "acknowledge_pet_visibility") acknowledgements.push(args);
    return command === "get_pet_visibility" ? true : undefined;
  }, { shouldMockEvents: true });
  const timers = new Map<number, { readonly delay: number | undefined; readonly run: () => void }>();
  let id = 0;
  const clock: VisibilityClock = {
    schedule: (run, delay) => { const token = ++id; timers.set(token, { delay, run }); return token; },
    cancel: (token) => { timers.delete(token); },
  };
  render(<Harness clock={clock} />);
  await act(async () => {});
    await act(async () => { await emit("deskmate://pet-visibility", { phase: "leaving", token: 1 }); });
    expect([...timers.values()].map((timer) => timer.delay)).toContain(910);
    await act(async () => { await emit("deskmate://pet-visibility", { phase: "reset", token: 2 }); });
    expect([...timers.values()].map((timer) => timer.delay)).not.toContain(910);
    await act(async () => { for (const timer of timers.values()) timer.run(); });
    expect(acknowledgements).toEqual([{ token: 2 }]);
});


for (const gif of [true, false]) test(`native visibility updates every renderer type (animated=${gif})`, async () => {
  mockIPC((command) => command === "get_pet_visibility" || command === "plugin:window|is_visible" ? true : undefined, { shouldMockEvents: true });
  const view = render(<Harness gif={gif} />);
  await act(async () => {});
  const output = view.container.querySelector("output");
  expect(output?.dataset.nativeVisible).toBe("true");
  await act(async () => { await emit("deskmate://pet-native-visibility", false); });
  expect(output?.dataset.nativeVisible).toBe("false");
  expect(output?.dataset.visible).toBe("false");
  await act(async () => { await emit("deskmate://pet-native-visibility", true); });
  expect(output?.dataset.nativeVisible).toBe("true");
  expect(output?.dataset.visible).toBe("true");
});

test("leaving/reset feedback cannot report that a physically hidden window is drawing", async () => {
  mockIPC((command) => command === "get_pet_visibility" || command === "plugin:window|is_visible" ? true : undefined, { shouldMockEvents: true });
  const view = render(<Harness />);
  await act(async () => {});
  const output = view.container.querySelector("output");
  await act(async () => { await emit("deskmate://pet-visibility", { phase: "leaving", token: 1 }); });
  expect(output?.dataset.visible).toBe("false");
  expect(output?.dataset.nativeVisible).toBe("true");
  await act(async () => { await emit("deskmate://pet-native-visibility", false); await emit("deskmate://pet-visibility", { phase: "reset", token: 2 }); });
  expect(output?.dataset.visible).toBe("true");
  expect(output?.dataset.nativeVisible).toBe("false");
  await act(async () => { await emit("deskmate://pet-native-visibility", true); });
  expect(output?.dataset.nativeVisible).toBe("true");
  expect(output?.dataset.leaving).toBe("false");
});

for (const visible of [true, false]) test(`late startup snapshot cannot overwrite a physical visibility event (${visible})`, async () => {
  let finish: ((value: boolean) => void) | undefined;
  mockIPC((command) => {
    if (command === "get_pet_visibility") return !visible;
    if (command === "plugin:window|is_visible") return new Promise<boolean>((resolve) => { finish = resolve; });
    return undefined;
  }, { shouldMockEvents: true });
  const view = render(<Harness />);
  await act(async () => {});
  expect(finish).toBeDefined();
  await act(async () => { await emit("deskmate://pet-native-visibility", visible); });
  await act(async () => { finish?.(!visible); });
  const output = view.container.querySelector("output");
  expect(output?.dataset.nativeVisible).toBe(String(visible));
  expect(output?.dataset.visible).toBe(String(visible));
});

test("old persona query and queued native callback cannot change a replacement persona", async () => {
  let finish: ((value: boolean) => void) | undefined;
  let calls = 0;
  mockIPC((command) => {
    if (command === "get_pet_visibility") return true;
    if (command === "plugin:window|is_visible") return ++calls === 1 ? new Promise<boolean>((resolve) => { finish = resolve; }) : false;
    return undefined;
  }, { shouldMockEvents: true });
  const view = render(<Harness />);
  await act(async () => {});
  view.rerender(<Harness personaId="b" gif={false} />);
  await act(async () => {});
  await act(async () => { finish?.(true); });
  expect(view.container.querySelector("output")?.dataset.nativeVisible).toBe("false");
});
