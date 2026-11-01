import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { act, cleanup, renderHook } from "@testing-library/react";
import * as core from "@tauri-apps/api/core";
import * as events from "@tauri-apps/api/event";
import * as opencode from "../lib/opencode";
const invoke = mock<(command: string, args?: unknown) => Promise<unknown>>(() => Promise.resolve(null));
let changed: (() => void) | undefined;
const getSessionMessages = mock<(_session: string, _directory?: string) => Promise<readonly { parts: readonly unknown[] }[]>>(() => Promise.resolve([]));
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
mock.module("@tauri-apps/api/event", () => ({ ...events, listen: (_event: string, callback: () => void) => { changed = callback; return Promise.resolve(() => {}); } }));
mock.module("../lib/opencode", () => ({ ...opencode, getSessionMessages }));
const { useWorklogChat } = await import("./useWorklogChat");

beforeEach(() => { invoke.mockReset(); getSessionMessages.mockReset(); getSessionMessages.mockResolvedValue([]); changed = undefined; });
afterEach(cleanup);

const requestId = "ae0e8d01-25f2-2d08-3217-8d2de99cbedf";
const toolOutput = (status: "pending" | "rejected") => ({
  tool: "worklog_update", messageID: "message", state: { output: JSON.stringify({ requestId, status,
    ...(status === "rejected" ? { error: { code: "NEEDS_EXPLICIT_REQUEST" } } : {}),
  }) },
});

const rejected = [{ messageId: "message", requestId, error: "NEEDS_EXPLICIT_REQUEST", undoable: false }];
test("opening uses the shared snapshot with zero extra full-session reads", async () => {
  invoke.mockResolvedValue([]);
  const hook = renderHook(() => useWorklogChat("reopened", "C:/project-b", "view:1", rejected));
  await act(async () => {});
  expect(getSessionMessages).not.toHaveBeenCalled();
  expect(invoke).toHaveBeenCalledWith("worklog_get_operation_states", { requestIds: [requestId] });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});
test("revisiting a directory cannot publish an older batch into the new view", async () => {
  let finish: (value: unknown) => void = () => {};
  invoke.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  invoke.mockResolvedValue([]);
  const hook = renderHook(({ directory, generation }: { directory: string; generation: number }) => useWorklogChat("same", directory, String(generation), generation === 1 ? rejected : undefined), { initialProps: { directory: "C:/one", generation: 1 } });
  await act(async () => {});
  hook.rerender({ directory: "C:/two", generation: 2 });
  hook.rerender({ directory: "C:/one", generation: 3 });
  await act(async () => { finish([{ requestId, receipt: { operationId: requestId, entityId: "old", entityKind: "entry", revision: 1, status: "committed" }, run: null, schedule: null, entityMissing: false }]); });
  expect(hook.result.current.operations).toEqual([]);
});
test("empty history and ordinary idle never read the whole conversation", async () => {
  const hook = renderHook(() => useWorklogChat("session", "C:/one", "key", []));
  await act(async () => { await hook.result.current.refreshKnown(); });
  expect(invoke).not.toHaveBeenCalled();
  expect(getSessionMessages).not.toHaveBeenCalled();
});

test("unrelated journal changes do not turn a rejected operation back into pending", async () => {
  invoke.mockResolvedValue([]);
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("rejected")); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
  await act(async () => { changed?.(); });
  expect(invoke).toHaveBeenCalledWith("worklog_get_operation_states", { requestIds: [requestId] });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});

test("late empty receipt lookup cannot erase a later rejection", async () => {
  let finish: (value: unknown) => void = () => {};
  invoke.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("pending")); });
  await act(async () => { hook.result.current.acceptTool(toolOutput("rejected")); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
  await act(async () => { finish([]); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});

test("a verified host receipt can resolve a previously pending operation", async () => {
  invoke.mockResolvedValue([]);
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("pending")); });
  const receipt = { operationId: requestId, entityId: "entry", entityKind: "entry", revision: 1, businessDate: "2026-10-27", status: "committed" as const };
  invoke.mockResolvedValue([{requestId,receipt,run:null,schedule:null,entityMissing:false}]);
  await act(async () => { changed?.(); });
  expect(hook.result.current.operations[0]?.receipt).toEqual(receipt);
  expect(hook.result.current.operations[0]?.error).toBeNull();
});

test("an old-session save completes without publishing a receipt into the new chat", async () => {
  let finishSave: (value: unknown) => void = () => {};
  const saveResponse = new Promise<unknown>((resolve) => { finishSave = resolve; });
  let operationLookups = 0;
  invoke.mockImplementation(async (command) => {
    if (command === "worklog_record") return saveResponse;
    if (command === "worklog_get_operation_states") operationLookups += 1;
    return null;
  });
  const hook = renderHook(({ session }: { readonly session: string }) => useWorklogChat(session), { initialProps: { session: "old-session" } });
  let saving = Promise.resolve();
  await act(async () => { saving = hook.result.current.save("old-message", "Synthetic old-session entry"); });
  expect(hook.result.current.operations.length).toBe(1);
  hook.rerender({ session: "new-session" });
  await act(async () => { finishSave({ status: "committed" }); await saving; });
  expect(hook.result.current.operations).toEqual([]);
  expect(operationLookups).toBe(0);
});

test("an old-session rejected schedule does not attach an error to the new chat", async () => {
  let rejectSchedule: (error: unknown) => void = () => {};
  const response = new Promise<unknown>((_, reject) => { rejectSchedule = reject; });
  invoke.mockImplementation(async (command) => command === "worklog_save_schedule" ? response : null);
  const hook = renderHook(({ session }: { readonly session: string }) => useWorklogChat(session), { initialProps: { session: "old-session" } });
  let saving = Promise.resolve();
  await act(async () => { saving = hook.result.current.schedule("old-message"); });
  hook.rerender({ session: "new-session" });
  await act(async () => { rejectSchedule({ code: "CONFLICT", message: "Changed" }); await saving; });
  expect(hook.result.current.operations).toEqual([]);
});
