import { catalogPageFixture, nativeHistoryFixture, registeredHistoryFixture } from "../testing/historyCatalogFixtures";
// allow: SIZE_OK — fixed-reply timing cases stay together to preserve shared fake-clock setup.
import {
  afterEach,
  beforeEach,
  expect,
  jest,
  mock,
  test,
} from "bun:test";
import * as tauriCore from "@tauri-apps/api/core";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
} from "@testing-library/react";
import type { PetActivityEvent } from "../lib/petState";
import type { OpenCodeEvent } from "../lib/opencode";

const invoke = mock<(command: string, args?: unknown) => Promise<unknown>>(
  () => Promise.resolve(undefined),
);
const listen = mock(() => Promise.resolve(() => {}));
let eventHandler: ((event: OpenCodeEvent) => void) | null = null;
const activityEvents: PetActivityEvent[] = [];
const originalFetch = globalThis.fetch;
let abortStatus: "idle" | "busy" = "idle";
let promptTransport: "ok" | "lost_confirmed" | "lost_unknown" | "lost_not_found" = "ok";
let submissionRead: (() => Promise<Response>) | null = null;
let failBeforePrompt = false;
let failNextCatalogAccess = false;
let registrationWait: (() => Promise<unknown>) | null = null;
let submittedMessageId: string | null = null;
let promptAttempts = 0;

mock.module("@tauri-apps/api/core", () => ({ ...tauriCore, invoke }));
mock.module("@tauri-apps/api/event", () => ({
  listen,
  emit: (name: string, payload: PetActivityEvent) => { if (name === "deskmate://pet-activity") activityEvents.push(payload); return Promise.resolve(); },
}));

function makeJsonResponse(body: unknown): Response {
  return new Response(JSON.stringify(body), {
    status: 200,
    headers: { "Content-Type": "application/json" },
  });
}

function installOpenCodeTransport(): void {
  const fetchMock = mock((input: RequestInfo | URL, init?: RequestInit) => {
    const url = new URL(String(input)).pathname;
    if (url.endsWith("/event")) {
      return Promise.resolve(
        new Response(
          new ReadableStream<Uint8Array>({
            start(controller) {
              eventHandler = (event: OpenCodeEvent) => {
                controller.enqueue(
                  new TextEncoder().encode(`data: ${JSON.stringify(event)}\n\n`),
                );
              };
            },
          }),
          { status: 200 },
        ),
      );
    }
    if (url.endsWith("/session") && init?.method === "GET") {
      return Promise.resolve(new Response(null, { status: 200 }));
    }
    if (url.endsWith("/session") && init?.method === "POST") {
      return Promise.resolve(makeJsonResponse({ id: "ses_fixed", title: "t", directory: "." }));
    }
    if (url.endsWith("/session/ses_fixed/prompt_async") && init?.method === "POST") {
      promptAttempts += 1;
      const body = typeof init.body === "string" ? JSON.parse(init.body) as unknown : null;
      submittedMessageId = typeof body === "object" && body !== null && "messageID" in body
        ? String(Reflect.get(body, "messageID"))
        : null;
      if (promptTransport !== "ok") return Promise.reject(new Error("response lost"));
      return Promise.resolve(new Response(null, { status: 204 }));
    }
    if (url.endsWith("/session/ses_fixed/abort") && init?.method === "POST") {
      return Promise.resolve(makeJsonResponse(true));
    }
    if (url.endsWith("/session/status")) {
      return Promise.resolve(makeJsonResponse({ ses_fixed: { type: abortStatus } }));
    }
    if (url.endsWith("/session/ses_fixed/message")) {
      if (submissionRead) return submissionRead();
      if (promptTransport === "lost_unknown") return Promise.reject(new Error("sidecar offline"));
      if (promptTransport === "lost_confirmed" && submittedMessageId) {
        return Promise.resolve(makeJsonResponse([{ info: {
          id: submittedMessageId,
          sessionID: "ses_fixed",
          role: "user",
        }, parts: [] }]));
      }
      return Promise.resolve(makeJsonResponse([]));
    }
    return Promise.resolve(new Response("unexpected opencode test request", { status: 500 }));
  });
  globalThis.fetch = Object.assign(fetchMock, {
    preconnect: originalFetch.preconnect,
  });
}

const { default: ChatApp } = await import("./ChatApp");

const SETTINGS = {
  autostart: false,
  language: "zh-CN",
  theme: "dark",
  providerId: "",
  modelId: "",
  yolo: false,
  baseUrl: "",
  apiKey: "",
  petScale: 1,
  outlineWidth: 0.0073,
  rimWidth: 0.4,
  rimIntensity: 1,
  specularIntensity: 0.5,
  petVisible: true,
  alwaysOnTop: true,
  scheduledTasks: [],
  shortcutToggleChat: "Ctrl+Alt+D",
  shortcutTogglePet: "",
  personaId: "xiaozhu",
  mouseFollow: false,
  userName: "",
  memoryAutoExtract: false,
  memoryAiUse: true,
  updateRepo: "clxgame/deskmate",
};

beforeEach(() => {
  activityEvents.length = 0;
  abortStatus = "idle";
  promptTransport = "ok";
  submissionRead = null;
  failBeforePrompt = false;
  failNextCatalogAccess = false;
  registrationWait = null;
  submittedMessageId = null;
  promptAttempts = 0;
  eventHandler = null;
  invoke.mockReset();
  invoke.mockImplementation((command: string) => {
    switch (command) {
      case "chat_model_resolve":
        return Promise.resolve({ configuredProviderId: "test-entry", sidecarId: "yume-2", modelId: "claude-sonnet-4.5", modelName: "Claude Sonnet 4.5" });
      case "history_model_selection_get":
        return Promise.resolve({ mode: "inherit" });
      case "history_model_selection_set":
        return Promise.resolve(undefined);
      case "history_recent_workspaces":
        return Promise.resolve([]);
      case "history_register_native_session":
        return Promise.resolve(registeredHistoryFixture({ sessionId: "ses_fixed", directory: "." }));
      case "history_catalog_access":
        if (failNextCatalogAccess) { failNextCatalogAccess = false; return Promise.reject(new Error("private internal access detail")); }
        return invoke("history_catalog_load").then((value: any) => value.entry);
      case "worklog_register_turn":
        if (failBeforePrompt) failNextCatalogAccess = true;
        return Promise.resolve();
      case "memory_register_turn":
        return registrationWait?.() ?? Promise.resolve();
    case "history_catalog_load":
        return Promise.resolve({ entry: nativeHistoryFixture("ses_fixed"), messages: [] });
      case "history_catalog_list":
        return Promise.resolve(catalogPageFixture([]));
      case "sidecar_base_url":
        return Promise.resolve("http://127.0.0.1:48888");
      case "get_settings":
        return Promise.resolve(SETTINGS);
      case "load_persona":
        return Promise.resolve({ persona: "你是小著。", skills: undefined, placeholders: null });
      case "memory_context":
        return Promise.resolve({ memories: [], promptBlock: "" });
      case "history_save":
        return Promise.resolve(undefined);
      case "chat_abort_session":
        return abortStatus === "busy"
          ? Promise.reject(new Error("agent_abort_still_running"))
          : Promise.resolve(undefined);
      default:
        return Promise.resolve(undefined);
    }
  });
  installOpenCodeTransport();
});

afterEach(() => {
  jest.useRealTimers();
  cleanup();
  globalThis.fetch = originalFetch;
});


async function sendPrompt() {
  render(<ChatApp />);
  const input = await screen.findByPlaceholderText("输入消息,Enter 发送");
  const send = screen.getByRole("button", { name: "发送" });
  fireEvent.change(input, { target: { value: "请回答一个测试问题" } });
  fireEvent.click(send);
  await waitFor(() => expect(activityEvents.some((event) => event.type === "start")).toBe(true));
  const start = activityEvents.find((event) => event.type === "start");
  if (!start) throw new Error("missing start event");
  return { id: "assistant", role: "assistant", sessionID: start.sessionId, parentID: start.requestId, time: { created: 1, completed: 2 }, finish: "stop" };
}

async function deliverEvents(events: OpenCodeEvent[]): Promise<void> {
  await act(async () => {
    for (const event of events) eventHandler?.(event);
    // The event subscription is a real async fetch stream: let the reader
    // drain before asserting.
    await new Promise((resolve) => setTimeout(resolve, 20));
  });
}

test("chat emits scoped success once after terminal assistant SSE even after idle", async () => {
  const info = await sendPrompt();
  await deliverEvents([{ type: "session.idle", properties: { sessionID: info.sessionID } }]);
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(0);
  await deliverEvents([
    { type: "message.updated", properties: { info } },
    { type: "message.updated", properties: { info } },
  ]);
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(1);
});

test("chat cancellation rejects a late terminal SSE", async () => {
  const info = await sendPrompt();
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  act(() => eventHandler?.({ type: "message.updated", properties: { info } }));
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(0);
});

test("chat keeps the run active when cancellation cannot be confirmed", async () => {
  // Given: the native server accepts abort but still reports the session busy.
  await sendPrompt();
  abortStatus = "busy";

  // When: the user requests stop.
  fireEvent.click(screen.getByRole("button", { name: "停" }));

  // Then: the stop control remains and no confirmed-cancel activity is emitted.
  await waitFor(() => expect(screen.getByRole("button", { name: "停" })).toBeDefined());
  expect(activityEvents.filter((event) => event.type === "cancel")).toHaveLength(0);
});

test("chat reconciles a lost prompt response without resending the turn", async () => {
  // Given: the sidecar stores the stable user message but its HTTP response is lost.
  promptTransport = "lost_confirmed";
  await sendPrompt();

  // When / Then: the native record confirms delivery and only one prompt attempt exists.
  await waitFor(() => expect(promptAttempts).toBe(1));
  expect(screen.getByRole("button", { name: "停" })).toBeDefined();
  expect(screen.queryByText(/不会自动重发/)).toBeNull();
});

test("chat leaves an unreadable prompt submission pending without resending", async () => {
  // Given: both the prompt response and subsequent native record lookup fail.
  promptTransport = "lost_unknown";
  await sendPrompt();

  // When / Then: the UI preserves a stoppable pending state and does not replay the prompt.
  await screen.findByText("消息可能已提交，但暂时无法读取原生记录；不会自动重发，请等待恢复或手动停止。");
  expect(screen.getByRole("button", { name: "停" })).toBeDefined();
  expect(promptAttempts).toBe(1);
});

test("chat session failure rejects a later success", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "session.error", properties: { sessionID: info.sessionID, error: { name: "TestFailure" } } },
    { type: "message.updated", properties: { info } },
  ]);
  expect(activityEvents.filter((event) => event.type === "error")).toHaveLength(1);
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(0);
});

test("fixed identity reply emits exactly one completion", async () => {
  render(<ChatApp />);
  const input = await screen.findByPlaceholderText("输入消息,Enter 发送");
  jest.useFakeTimers();
  await act(async () => {
    fireEvent.change(input, { target: { value: "你是谁？" } });
    fireEvent.click(screen.getByRole("button", { name: "发送" }));
    await Promise.resolve(); await Promise.resolve();
  });
  await act(async () => { jest.advanceTimersByTime(1000); await Promise.resolve(); });
  await act(async () => { jest.advanceTimersByTime(1000); await Promise.resolve(); });
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(1);
});

test("real SSE tool stop cannot celebrate before the tool follow-up response", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, time: { created: 1 }, finish: undefined } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, id: "part-tool", type: "tool", tool: "bash", state: { status: "completed", title: "test" } } } },
    { type: "message.updated", properties: { info } },
  ]);
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(0);
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, id: "final" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
  ]);
  expect(activityEvents.filter((event) => event.type === "success")).toHaveLength(1);
});

test("stop isolates late text, busy and error events for the cancelled request", async () => {
  const info = await sendPrompt();
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  await deliverEvents([
    { type: "message.updated", properties: { info } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "stale cancelled text" } } },
    { type: "session.status", properties: { sessionID: info.sessionID, status: { type: "busy" } } },
    { type: "session.error", properties: { sessionID: info.sessionID, error: { name: "stale cancelled failure" } } },
  ]);
  expect(screen.queryByText("stale cancelled text")).toBeNull();
  expect(screen.queryByText(/stale cancelled failure/)).toBeNull();
  expect(screen.queryByRole("button", { name: "停" })).toBeNull();
});

test("a request with an error before its first chunk rejects later text", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "session.error", properties: { sessionID: info.sessionID, error: { name: "first chunk failure" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "text after error" } } },
  ]);
  expect(screen.getByText(/first chunk failure/)).toBeDefined();
  expect(screen.queryByText("text after error")).toBeNull();
  expect(screen.queryByRole("button", { name: "停" })).toBeNull();
});

test("unmount cancels the scoped request and ignores late streamed content", async () => {
  const info = await sendPrompt();
  cleanup();
  await deliverEvents([
    { type: "message.updated", properties: { info } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "text after unmount" } } },
  ]);
  expect(activityEvents.filter(event => event.type === "cancel")).toHaveLength(1);
  expect(activityEvents.filter(event => event.type === "success")).toHaveLength(0);
  expect(screen.queryByText("text after unmount")).toBeNull();
});

test("a new request in the same session rejects text from a known old parent", async () => {
  const old = await sendPrompt();
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  fireEvent.change(screen.getByPlaceholderText("输入消息,Enter 发送"), { target: { value: "第二个普通问题" } });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await waitFor(() => expect(activityEvents.filter(event => event.type === "start")).toHaveLength(2));
  const current = activityEvents.filter(event => event.type === "start")[1]!;
  await deliverEvents([
    { type: "message.updated", properties: { info: old } },
    { type: "message.part.updated", properties: { part: { sessionID: old.sessionID, messageID: old.id, type: "text", text: "old parent text" } } },
    { type: "message.updated", properties: { info: { ...old, id: "current", parentID: current.requestId, time: { created: 3 }, finish: undefined } } },
    { type: "message.part.updated", properties: { part: { sessionID: old.sessionID, messageID: "current", type: "text", text: "current reply text" } } },
  ]);
  expect(screen.queryByText("old parent text")).toBeNull();
  expect(screen.getByText("current reply text")).toBeDefined();
  expect(activityEvents.filter(event => event.type === "success")).toHaveLength(0);
});


test("a queued second snapshot flushes before idle and preserves ordered parts", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, time: { created: 1 } } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, id: "p1", type: "text", text: "first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, id: "p1", type: "text", text: "first final " } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, id: "p2", type: "text", text: "🚀 second" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
  ]);
  expect(screen.getByText("first final 🚀 second")).toBeDefined();
  expect(document.querySelectorAll('[data-message-id="assistant"]')).toHaveLength(1);
  expect(document.querySelector(".chat-typing")).toBeNull();
});

test("native text deltas render before completion and final snapshots replace accumulated text", async () => {
  const info = await sendPrompt();
  const identity = { sessionID: info.sessionID, messageID: info.id };
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, time: { created: 1 } } } },
    { type: "message.part.updated", properties: { part: { ...identity, id: "text", type: "text", text: "" } } },
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: "visible while streaming" } },
  ]);
  expect(screen.getByText("visible while streaming")).toBeDefined();
  expect(screen.getByRole("button", { name: "停" })).toBeDefined();
  await deliverEvents([
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: " pending" } },
    { type: "message.part.updated", properties: { part: { ...identity, id: "text", type: "text", text: "authoritative final text" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: " late" } },
  ]);
  expect(screen.getByText("authoritative final text")).toBeDefined();
  expect(screen.queryByText(/pending|late/)).toBeNull();
});

test("deltas cannot reveal reasoning or cross session, user, parent and Stop boundaries", async () => {
  const info = await sendPrompt();
  const identity = { sessionID: info.sessionID, messageID: info.id };
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, time: { created: 1 } } } },
    { type: "message.part.updated", properties: { part: { ...identity, id: "text", type: "text", text: "safe" } } },
    { type: "message.part.updated", properties: { part: { ...identity, id: "reason", type: "reasoning", text: "" } } },
    { type: "message.part.delta", properties: { ...identity, partID: "reason", field: "text", delta: " secret reasoning" } },
    { type: "message.part.delta", properties: { ...identity, sessionID: "other", partID: "text", field: "text", delta: " wrong session" } },
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "output", delta: " wrong field" } },
    { type: "message.updated", properties: { info: { ...info, id: "user", role: "user" } } },
    { type: "message.part.delta", properties: { sessionID: info.sessionID, messageID: "user", partID: "text", field: "text", delta: " user echo" } },
    { type: "message.updated", properties: { info: { ...info, parentID: "old-request", time: { created: 1 } } } },
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: " wrong parent" } },
    { type: "message.updated", properties: { info: { ...info, parentID: submittedMessageId, time: { created: 1 } } } },
    { type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: " accepted" } },
  ]);
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  await deliverEvents([{ type: "message.part.delta", properties: { ...identity, partID: "text", field: "text", delta: " cancelled tail" } }]);
  expect(screen.getByText("safe accepted")).toBeDefined();
  expect(document.querySelector(".chat-msg-assistant")?.textContent).not.toMatch(/secret|wrong|user echo|cancelled/);
});

test("error flushes queued content before its error row and rejects later chunks", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "legal pending tail" } } },
    { type: "session.error", properties: { sessionID: info.sessionID, error: { name: "stream failed" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "stale after error" } } },
  ]);
  const rows = document.querySelectorAll(".chat-msg-assistant");
  expect(rows[0]?.textContent).toContain("legal pending tail");
  expect(rows[1]?.textContent).toContain("stream failed");
  expect(screen.queryByText("stale after error")).toBeNull();
});

test("confirmed Stop keeps queued legal content and a new request never receives its stale timer", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.updated", properties: { info: { ...info, time: { created: 1 } } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "before stop final" } } },
  ]);
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  expect(screen.getByText("before stop final")).toBeDefined();
  fireEvent.change(screen.getByPlaceholderText("输入消息,Enter 发送"), { target: { value: "retry with a new turn" } });
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await waitFor(() => expect(promptAttempts).toBe(2));
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "stale retry tail" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: "new-assistant", type: "text", text: "current retry text" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
  ]);
  expect(screen.getByText("before stop final")).toBeDefined();
  expect(screen.getByText("current retry text")).toBeDefined();
  expect(screen.queryByText("stale retry tail")).toBeNull();
});

test("tool events flush prior text and duplicate text after a tool clears only plain activity", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "initial" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "same text" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, id: "tool", type: "tool", tool: "bash", state: { status: "running", title: "running command" } } } },
  ]);
  expect(screen.getByText("same text")).toBeDefined();
  expect(screen.getByText("running command")).toBeDefined();
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "same text" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
  ]);
  expect(screen.queryByText("running command")).toBeNull();
  expect(screen.getByText("same text")).toBeDefined();
});


test("an unconfirmed Stop leaves queued content and subsequent chunks live", async () => {
  const info = await sendPrompt();
  abortStatus = "busy";
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "pending before failed Stop" } } },
  ]);
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(invoke.mock.calls.some(([command]) => command === "chat_abort_session")).toBe(true));
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "still live after failed Stop" } } },
    { type: "session.idle", properties: { sessionID: info.sessionID } },
  ]);
  expect(screen.getByText("still live after failed Stop")).toBeDefined();
  expect(activityEvents.filter(event => event.type === "cancel")).toHaveLength(0);
});

test("unmount discards an already queued presentation timer", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "visible first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "queued before unmount" } } },
  ]);
  cleanup();
  await act(async () => { await new Promise(resolve => setTimeout(resolve, 50)); });
  expect(screen.queryByText("queued before unmount")).toBeNull();
  expect(activityEvents.filter(event => event.type === "cancel")).toHaveLength(1);
});


test("completed assistant metadata flushes queued text without waiting for session idle", async () => {
  const info = await sendPrompt();
  await deliverEvents([
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "first" } } },
    { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "metadata terminal final" } } },
    { type: "message.updated", properties: { info } },
  ]);
  expect(screen.getByText("metadata terminal final")).toBeDefined();
  // Text publication does not change the existing idle-confirmed celebration contract.
  expect(activityEvents.filter(event => event.type === "success")).toHaveLength(0);
  expect(screen.getByRole("button", { name: "停" })).toBeDefined();
});


test("confirmed missing prompt removes its optimistic row, keeps the draft and allows one visible retry", async () => {
  promptTransport = "lost_not_found";
  await sendPrompt();
  await screen.findByRole("alert");
  expect(screen.getByRole("alert").textContent).toBe("消息发送失败，草稿已保留，请重试。");
  expect(document.querySelectorAll(".chat-msg-user")).toHaveLength(0);
  expect((screen.getByPlaceholderText("输入消息,Enter 发送") as HTMLTextAreaElement).value).toBe("请回答一个测试问题");
  expect(promptAttempts).toBe(1);
  promptTransport = "ok";
  fireEvent.click(screen.getByRole("button", { name: "发送" }));
  await waitFor(() => expect(promptAttempts).toBe(2));
  expect(document.querySelectorAll(".chat-msg-user")).toHaveLength(1);
  expect(screen.queryByRole("alert")).toBeNull();
  expect((screen.getByPlaceholderText("输入消息,Enter 发送") as HTMLTextAreaElement).value).toBe("");
});

test("a failure before prompt submission removes only its optimistic question and shows no internal detail", async () => {
  failBeforePrompt = true;
  await sendPrompt();
  await screen.findByRole("alert");
  expect(screen.getByRole("alert").textContent).toBe("消息发送失败，草稿已保留，请重试。");
  expect(document.querySelectorAll(".chat-msg-user")).toHaveLength(0);
  expect((screen.getByPlaceholderText("输入消息,Enter 发送") as HTMLTextAreaElement).value).toBe("请回答一个测试问题");
  expect(promptAttempts).toBe(0);
  expect(screen.queryByText(/private internal access detail/)).toBeNull();
});

test("late unknown submission confirmation cannot revive a request after confirmed Stop", async () => {
  promptTransport = "lost_unknown";
  let finishConfirmation: (() => void) | undefined;
  submissionRead = () => new Promise((_resolve, reject) => { finishConfirmation = () => reject(new Error("unavailable")); });
  await sendPrompt();
  await waitFor(() => expect(finishConfirmation).toBeDefined());
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  await act(async () => { finishConfirmation?.(); await Promise.resolve(); await Promise.resolve(); });
  expect(screen.queryByText(/消息可能已提交/) === null).toBe(true);
  expect(screen.queryByRole("alert")).toBeNull();
  expect(screen.queryByRole("button", { name: "停" })).toBeNull();
  expect(document.querySelectorAll(".chat-msg-user")).toHaveLength(1);
});


test("Stop during native turn registration prevents a later prompt submission", async () => {
  let finishRegistration: (() => void) | undefined;
  registrationWait = () => new Promise(resolve => { finishRegistration = () => resolve(undefined); });
  await sendPrompt();
  await waitFor(() => expect(finishRegistration).toBeDefined());
  fireEvent.click(screen.getByRole("button", { name: "停" }));
  await waitFor(() => expect(activityEvents.some(event => event.type === "cancel")).toBe(true));
  await act(async () => { finishRegistration?.(); await Promise.resolve(); await Promise.resolve(); });
  expect(promptAttempts).toBe(0);
  expect(screen.queryByRole("button", { name: "停" }) === null).toBe(true);
  expect(screen.queryByRole("alert") === null).toBe(true);
});


for (const result of ["confirmed", "unknown", "not_found"] as const) {
  test(`native completion before ${result} confirmation never restores the sent draft`, async () => {
    promptTransport = "lost_not_found";
    let finishConfirmation: (() => void) | undefined;
    submissionRead = () => {
      // Idle independently recovers worklog receipts; only the submission read is delayed.
      if (finishConfirmation) return Promise.resolve(makeJsonResponse([]));
      return new Promise((resolve, reject) => {
        finishConfirmation = () => result === "unknown"
          ? reject(new Error("temporarily unreadable"))
          : resolve(makeJsonResponse(result === "confirmed" ? [{ info: { id: submittedMessageId, sessionID: "ses_fixed", role: "user" }, parts: [] }] : []));
      });
    };
    const info = await sendPrompt();
    await waitFor(() => expect(finishConfirmation).toBeDefined());
    await deliverEvents([
      { type: "message.updated", properties: { info: { ...info, time: { created: 1 }, finish: undefined } } },
      { type: "message.part.updated", properties: { part: { sessionID: info.sessionID, messageID: info.id, type: "text", text: "native completed answer" } } },
      { type: "message.updated", properties: { info } },
      { type: "session.idle", properties: { sessionID: info.sessionID } },
    ]);
    expect(activityEvents.filter(event => event.type === "success")).toHaveLength(1);
    await act(async () => { finishConfirmation?.(); await Promise.resolve(); await Promise.resolve(); });
    const input = screen.getByPlaceholderText("输入消息,Enter 发送") as HTMLTextAreaElement;
    await waitFor(() => expect(input.disabled).toBe(false));
    expect(input.value).toBe("");
    expect(document.querySelectorAll(".chat-msg-user")).toHaveLength(1);
    expect(screen.getByText("native completed answer").textContent).toBe("native completed answer");
    expect(screen.queryByRole("alert") === null).toBe(true);
    expect(screen.queryByText(/消息可能已提交/) === null).toBe(true);
    expect(promptAttempts).toBe(1);
  });
}
