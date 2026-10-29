import { expect, test } from "bun:test";
import { createWorklogRecovery, type ChatWorklogOperation } from "./worklogRecovery";
import type { OperationState } from "../lib/worklog";

const id = (n: number) => `00000000-0000-4000-8000-${String(n).padStart(12, "0")}`;
const outcome = (n: number, error: string | null = null) => ({ messageId: `message-${n}`, requestId: id(n), error, undoable: true });
const saved = (n: number): OperationState => ({ requestId: id(n), receipt: { operationId: id(n), entityKind: "entry", entityId: "entry", revision: 1, businessDate: null, status: "committed" }, run: null, schedule: null, entityMissing: false });

test("zero operations do no IO; one hundred restore in exactly two bounded batches", async () => {
  const batches: string[][] = [];
  let current: readonly ChatWorklogOperation[] = [];
  const store = createWorklogRecovery(rows => { current = rows; }, async ids => {
    batches.push([...ids]);
    return ids.map(requestId => ({ ...saved(1), requestId, receipt: null }));
  });
  await store.refresh();
  expect(batches).toHaveLength(0);
  store.accept(Array.from({ length: 100 }, (_, n) => outcome(n)));
  await store.refresh();
  expect(batches.map(batch => batch.length)).toEqual([50, 50]);
  expect(current).toHaveLength(100);
});

test("an empty or old lookup never erases a later explicit rejection", async () => {
  let complete: (states: readonly OperationState[]) => void = () => {};
  let calls = 0;
  let current: readonly ChatWorklogOperation[] = [];
  const store = createWorklogRecovery(rows => { current = rows; }, async () => {
    calls += 1;
    return calls === 1 ? new Promise(resolve => { complete = resolve; }) : [];
  });
  store.accept([outcome(1)]);
  await Promise.resolve();
  store.accept([outcome(1, "NEEDS_EXPLICIT_REQUEST")]);
  complete([saved(1)]);
  await store.refresh();
  expect(current[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
  expect(current[0]?.receipt).toBeNull();
});

test("only an exact host receipt confirms a result and deletion is refreshed", async () => {
  let response = saved(1);
  const store = createWorklogRecovery(() => {}, async () => [response]);
  store.accept([outcome(1, "NEEDS_EXPLICIT_REQUEST")]);
  await store.refresh();
  expect(store.get(id(1))?.error).toBeNull();
  response = { ...response, receipt: { ...response.receipt!, status: "deleted" } };
  await store.refresh();
  expect(store.get(id(1))?.receipt?.status).toBe("deleted");
});

test("a closed view cannot publish late responses or queue further IO", async () => {
  let complete: (states: readonly OperationState[]) => void = () => {};
  let publications = 0, calls = 0;
  const store = createWorklogRecovery(() => { publications += 1; }, () => { calls += 1; return new Promise(resolve => { complete = resolve; }); });
  store.accept([outcome(1)]);
  await Promise.resolve();
  store.close();
  const before = publications;
  complete([saved(1)]);
  await store.refresh();
  expect(publications).toBe(before);
  expect(calls).toBe(1);
});

test("duplicate changes while a lookup runs coalesce into one follow-up", async () => {
  let complete: (states: readonly OperationState[]) => void = () => {};
  let calls = 0;
  const store = createWorklogRecovery(() => {}, async () => {
    calls += 1;
    return calls === 1 ? new Promise(resolve => { complete = resolve; }) : [saved(1)];
  });
  store.accept([outcome(1)]);
  await Promise.resolve();
  for (let i = 0; i < 50; i++) void store.refresh();
  complete([saved(1)]);
  await store.refresh();
  expect(calls).toBe(2);
  expect(store.get(id(1))?.receipt?.status).toBe("committed");
});
