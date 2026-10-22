import { afterEach, beforeEach, expect, test } from "bun:test";
import { emit } from "@tauri-apps/api/event";
import { clearMocks, mockIPC } from "@tauri-apps/api/mocks";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { createChatPetActivity } from "../chat/petActivity";
import { broadcastPetActivity, createPetActivitySequencer, isPetActivityEvent, onPetActivity, type PetActivityEvent } from "./petState";
import { initialGifState, reduceGifState } from "../pet/gifState";

beforeEach(() => { restoreTauriModuleFixture(); mockIPC(() => undefined, { shouldMockEvents: true }); });
afterEach(clearMocks);
const scope = { sessionId: "chat", requestId: "message" };

test("chat and agent producers share increasing ordering and old run completion cannot overwrite chat", async () => {
  const events: PetActivityEvent[] = [];
  const stop = await onPetActivity(event => events.push(event));
  const chat = createChatPetActivity(broadcastPetActivity);
  chat.start(scope);
  chat.mood("talking");
  broadcastPetActivity({ sessionId: "agent", requestId: "run", type: "start" });
  chat.start({ ...scope, requestId: "next" });
  broadcastPetActivity({ sessionId: "agent", requestId: "run", type: "success" });
  chat.mood("talking");
  await Promise.resolve();
  expect(events.length).toBe(7);
  expect(new Set(events.map(event => event.producerEpoch)).size).toBe(1);
  expect(events.every((event, index) => index === 0 || event.sequence === events[index - 1]!.sequence + 1)).toBe(true);
  const state = events.reduce((value, event) => reduceGifState(value, event, 0), initialGifState);
  expect(state.request).toEqual({ ...scope, requestId: "next" });
  expect(state.base).toBe("talking");
  expect(state.terminal).toBe(false);
  stop();
});

test("the sequencer only touches one storage key at creation and survives a backward clock on recreation", () => {
  let stored: string | null = "9999999999999";
  let reads = 0, writes = 0;
  const storage = { getItem: () => { reads++; return stored; }, setItem: (_key: string, value: string) => { writes++; stored = value; } };
  const first = createPetActivitySequencer(() => storage, () => 1000);
  let previous = first({ ...scope, type: "start" });
  const epoch = previous.producerEpoch;
  for (let index = 0; index < 10_000; index++) previous = first({ ...scope, type: "mood", mood: "talking" });
  expect(previous.sequence).toBe(10_001);
  expect(reads).toBe(1);
  expect(writes).toBe(1);
  const reloaded = createPetActivitySequencer(() => storage, () => 1)({ ...scope, requestId: "new", type: "start" });
  expect(reloaded.producerEpoch).toBe(epoch + 1);
  expect(reloaded.sequence).toBe(1);
});

test("disabled storage does not interrupt in-page ordering", () => {
  const unavailable = () => { throw new Error("storage disabled"); };
  const first = createPetActivitySequencer(unavailable, () => 10);
  const a = first({ ...scope, type: "start" });
  const b = first({ ...scope, type: "success" });
  const recreated = createPetActivitySequencer(unavailable, () => 1)({ ...scope, requestId: "next", type: "start" });
  expect(b.sequence).toBe(a.sequence + 1);
  expect(recreated.producerEpoch).toBeGreaterThan(a.producerEpoch);
});

test("cross-window payload validation ignores malformed and unsequenced events", async () => {
  const events: PetActivityEvent[] = [];
  const stop = await onPetActivity(event => events.push(event));
  const valid: PetActivityEvent = { ...scope, type: "start", producerEpoch: 1, sequence: 1, eventId: "1:1" };
  for (const value of [null, {}, { ...valid, sequence: NaN }, { ...valid, sequence: Infinity }, { ...valid, sequence: Number.MAX_SAFE_INTEGER + 1 },
    { ...valid, sequence: -1 }, { ...valid, producerEpoch: 0 }, { ...valid, eventId: "different" },
    { ...valid, sessionId: "" }, { ...valid, type: "mood", mood: { toString: "not callable" } }, { ...valid, type: "mood", mood: "invalid" }, { ...valid, type: "other" }]) {
    expect(isPetActivityEvent(value)).toBe(false);
    await emit("deskmate://pet-activity", value);
  }
  await emit("deskmate://pet-activity", valid);
  expect(events).toEqual([valid]);
  stop();
});
