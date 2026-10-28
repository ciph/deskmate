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

test("opening a native session restores its rejected journal receipt without another reply", async () => {
  getSessionMessages.mockResolvedValue([{ parts: [toolOutput("rejected")] }]);
  const hook = renderHook(() => useWorklogChat("reopened"));
  await act(async () => {});
  expect(getSessionMessages).toHaveBeenCalledWith("reopened", undefined);
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});

test("receipt recovery reads the selected native directory", async () => {
  renderHook(() => useWorklogChat("reopened", "C:/project-b"));
  await act(async () => {});
  expect(getSessionMessages).toHaveBeenCalledWith("reopened", "C:/project-b");
});

test("revisiting a session cannot publish its previous recovery after switching directories", async () => {
  let finish: (value: readonly { parts: readonly unknown[] }[]) => void = () => {};
  getSessionMessages.mockImplementationOnce(() => new Promise(resolve => { finish = resolve; }));
  const hook = renderHook(({ directory }: { directory: string }) => useWorklogChat("same-id", directory), {
    initialProps: { directory: "C:/first" },
  });
  hook.rerender({ directory: "C:/second" });
  hook.rerender({ directory: "C:/first" });
  await act(async () => { finish([{ parts: [toolOutput("rejected")] }]); });
  expect(hook.result.current.operations).toEqual([]);
});

test("a late history recovery cannot publish receipts into another session", async () => {
  let finish: (value: readonly { parts: readonly unknown[] }[]) => void = () => {};
  getSessionMessages.mockImplementation(session => session === "old" ? new Promise(resolve => { finish = resolve; }) : Promise.resolve([]));
  const hook = renderHook(({session}: {session: string}) => useWorklogChat(session), {initialProps: {session: "old"}});
  hook.rerender({session: "new"});
  await act(async () => { finish([{parts: [toolOutput("rejected")]}]); });
  expect(hook.result.current.operations).toEqual([]);
});

test("unrelated journal changes do not turn a rejected operation back into pending", async () => {
  invoke.mockResolvedValue(null);
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("rejected")); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
  await act(async () => { changed?.(); });
  expect(invoke).toHaveBeenCalledWith("worklog_get_operation", { requestId });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});

test("late empty receipt lookup cannot erase a later rejection", async () => {
  let finish: (value: unknown) => void = () => {};
  invoke.mockImplementation(() => new Promise(resolve => { finish = resolve; }));
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("pending")); });
  await act(async () => { hook.result.current.acceptTool(toolOutput("rejected")); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
  await act(async () => { finish(null); });
  expect(hook.result.current.operations[0]?.error).toBe("NEEDS_EXPLICIT_REQUEST");
});

test("a verified host receipt can resolve a previously pending operation", async () => {
  invoke.mockResolvedValue(null);
  const hook = renderHook(() => useWorklogChat("session"));
  await act(async () => { hook.result.current.acceptTool(toolOutput("pending")); });
  const receipt = { operationId: requestId, entityId: "entry", entityKind: "entry", revision: 1, businessDate: "2026-10-27", status: "committed" as const };
  invoke.mockResolvedValue(receipt);
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
    if (command === "worklog_get_operation") operationLookups += 1;
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
