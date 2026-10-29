import { expect, test } from "bun:test";
import { createConversationEvents } from "./conversationEvents";
import type { OpenCodeEvent, subscribeEvents } from "../lib/opencode";
const scope = { key: "native-key", directory: "C:/one", sessionId: "session", generation: 1 };

test("subscribes before snapshot, buffers until commit and recovers only after reconnect", async () => {
  let receive: (event: OpenCodeEvent) => void = () => {};
  let connected: (reconnected: boolean) => void = () => {};
  const seen: string[] = [];
  let recoveries = 0, stopped = 0;
  const subscribe: typeof subscribeEvents = async (event, _directory, ready) => {
    receive = event; connected = ready!;
    queueMicrotask(() => connected(false));
    return () => { stopped += 1; };
  };
  const stream = createConversationEvents(event => seen.push(event.type), async () => { recoveries += 1; }, subscribe);
  await stream.prepare(scope);
  receive({ type: "session.status", properties: {} });
  expect(seen).toEqual([]);
  stream.flush(scope);
  expect(seen).toEqual(["session.status"]);
  receive({ type: "session.idle", properties: {} });
  expect(recoveries).toBe(0);
  connected(true);
  await Promise.resolve();
  expect(recoveries).toBe(1);
  stream.close();
  receive({ type: "late", properties: {} });
  expect(seen).not.toContain("late");
  expect(stopped).toBe(1);
});

test("revisiting the same identity rejects the previous connection generation", async () => {
  const callbacks: ((event: OpenCodeEvent) => void)[] = [];
  const seen: string[] = [];
  const stream = createConversationEvents(event => seen.push(event.type), async () => {}, async (event, _directory, ready) => {
    callbacks.push(event); queueMicrotask(() => ready?.(false)); return () => {};
  });
  await stream.prepare(scope);
  const next = { ...scope, generation: 2 };
  await stream.prepare(next);
  callbacks[0]!({ type: "old", properties: {} });
  callbacks[1]!({ type: "session.status", properties: {} });
  stream.flush(next);
  expect(seen).toEqual(["session.status"]);
  stream.close();
});

test("snapshot gaps discard ambiguous deltas and settle once at idle; other sessions never dirty the view", async () => {
  let receive: (event: OpenCodeEvent) => void = () => {};
  let reads = 0; const seen: string[] = [];
  const stream = createConversationEvents(event => seen.push(event.type), async () => { reads += 1; }, async (event, _dir, ready) => { receive = event; queueMicrotask(() => ready?.(false)); return () => {}; });
  await stream.prepare(scope);
  receive({ type: "message.part.delta", properties: { sessionID: "another", delta: "irrelevant" } });
  stream.flush(scope);
  receive({ type: "session.idle", properties: { sessionID: scope.sessionId } });
  expect(reads).toBe(0);
  stream.close(); await stream.prepare({ ...scope, generation: 2 });
  for (let i = 0; i < 10000; i++) receive({ type: "message.part.delta", properties: { sessionID: scope.sessionId, delta: "already in snapshot" } });
  stream.flush({ ...scope, generation: 2 });
  expect(seen).not.toContain("message.part.delta");
  receive({ type: "session.idle", properties: { sessionID: scope.sessionId } });
  await Promise.resolve(); expect(reads).toBe(1);
  stream.flush({ ...scope, generation: 2 });
  expect(seen.filter(type => type === "session.idle").length).toBe(2);
  stream.close();
});
