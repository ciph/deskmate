import { renderMarkdown as render } from "../testing/renderMarkdown";
import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import type { ComponentProps } from "react";
import { cleanup, fireEvent } from "@testing-library/react";
import { dict } from "../lib/i18n";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import { ChatMessageRow, type TextChatMessage } from "./ChatMessageRow";
import type { ChatWorklogOperation } from "./useWorklogChat";

beforeEach(restoreTauriModuleFixture);
afterEach(cleanup);

const NO_AVATARS: readonly string[] = [];
const NO_OPERATIONS: readonly ChatWorklogOperation[] = [];
const ignoreOperation = async () => {};
const copy = dict("zh-CN");
const markdown = "说明\n\n```ts\n  const x = 1;\n```\n\n|名称|状态|\n|---|---|\n|任务|完成|";

function propsFor(message: TextChatMessage): ComponentProps<typeof ChatMessageRow> {
  return {
    message, avatarSources: NO_AVATARS, lang: "zh-CN", resourceLanguage: "zh-CN", t: copy,
    currentDirectory: undefined, currentSessionId: null, streaming: false,
    operations: NO_OPERATIONS, onUndo: ignoreOperation, onRefresh: ignoreOperation,
  };
}

describe("ChatMessageRow", () => {
  test("skips unchanged row rendering and retains code/table focus and scroll during unrelated updates", async () => {
    let textReads = 0;
    const message: TextChatMessage = {
      id: "completed", role: "assistant",
      get text() { textReads += 1; return markdown; },
    };
    const props = propsFor(message);
    function Timeline({ counter }: { readonly counter: number }) {
      return <><span data-counter={counter}>{counter}</span><ChatMessageRow {...props} /></>;
    }
    const { container, rerender } = await render(<Timeline counter={0} />);
    const row = container.querySelector<HTMLElement>("[data-message-id]")!;
    const code = row.querySelector<HTMLPreElement>("pre")!;
    const table = row.querySelector<HTMLElement>(".chat-markdown-table")!;
    code.scrollLeft = 73;
    table.scrollLeft = 51;
    code.focus();
    const readsAfterMount = textReads;
    expect(readsAfterMount).toBeGreaterThan(0);
    const observer = new MutationObserver(() => {});
    observer.observe(row, { attributes: true, childList: true, subtree: true, characterData: true });
    try {
      for (let counter = 1; counter <= 20; counter += 1) await rerender(<Timeline counter={counter} />);
      expect(container.querySelector("[data-counter]")?.textContent).toBe("20");
      expect(textReads).toBe(readsAfterMount);
      expect(observer.takeRecords()).toHaveLength(0);
      expect(container.querySelector("[data-message-id]")).toBe(row);
      expect(row.querySelector("pre")).toBe(code);
      expect(row.querySelector(".chat-markdown-table")).toBe(table);
      expect(document.activeElement).toBe(code);
      expect(code.scrollLeft).toBe(73);
      expect(table.scrollLeft).toBe(51);
    } finally {
      observer.disconnect();
    }
  });

  test("streaming text and terminal presentation keep existing code/table nodes and their scroll positions", async () => {
    const message: TextChatMessage = { id: "stream", role: "assistant", text: markdown + "\n\n**开始" };
    const props = { ...propsFor(message), streaming: true };
    const { container, rerender } = await render(<ChatMessageRow {...props} />);
    const row = container.querySelector("[data-message-id]");
    const code = container.querySelector<HTMLPreElement>("pre")!;
    const table = container.querySelector<HTMLElement>(".chat-markdown-table")!;
    code.scrollLeft = 42;
    table.scrollLeft = 67;
    code.focus();
    await rerender(<ChatMessageRow {...props} message={{ ...message, text: markdown + "\n\n**开始继续" }} />);
    expect(container.querySelector("[data-message-id]")).toBe(row);
    expect(container.querySelector("pre")).toBe(code);
    expect(container.querySelector(".chat-markdown-table")).toBe(table);
    expect(container.querySelector("strong")?.textContent).toBe("开始继续");
    expect(document.activeElement).toBe(code);
    expect(code.scrollLeft).toBe(42);
    expect(table.scrollLeft).toBe(67);
    table.focus();
    await rerender(<ChatMessageRow {...props} streaming={false} message={{ ...message, text: markdown + "\n\n**开始继续并完成**" }} />);
    expect(container.querySelector("pre")).toBe(code);
    expect(container.querySelector(".chat-markdown-table")).toBe(table);
    expect(document.activeElement).toBe(table);
    expect(code.scrollLeft).toBe(42);
    expect(table.scrollLeft).toBe(67);
    expect(container.querySelector("strong")?.textContent).toBe("开始继续并完成");
  });

  test("preserves literal user copy, attachments and resource cards without exposing resource context", async () => {
    const message: TextChatMessage = {
      id: "user", role: "user",
      text: "**原文**\n\n<yume-local-resources>\nprivate context\n</yume-local-resources>",
      attachments: [
        { id: "image", name: "image.png", kind: "image", mime: "image/png", size: 1, status: "ready", dataUrl: "data:image/png;base64,AA==" },
        { id: "doc", name: "notes.txt", kind: "text", mime: "text/plain", size: 4, status: "ready", dataUrl: "data:text/plain;base64,dGVzdA==", truncated: false },
      ],
      resources: [{ id: "resource", name: "archive.zip", kind: "file", mime: "application/zip", size: 8, previewUrl: null }],
    };
    const { container, getByRole } = await render(<ChatMessageRow {...propsFor(message)} avatarSources={["data:image/png;base64,AAAA"]} />);
    expect(container.querySelector(".chat-bubble")?.textContent).toBe("**原文**");
    expect(container.querySelector("strong")).toBeNull();
    expect(container.textContent).not.toContain("private context");
    expect(container.querySelector(".chat-attachment-image")?.getAttribute("alt")).toBe("image.png");
    expect(container.querySelector(".chat-attachment-document")?.textContent).toBe("文档 · notes.txt");
    expect(getByRole("article", { name: "archive.zip" })).toBeTruthy();
    expect(getByRole("img", { name: copy.myAvatar }).querySelector("img")?.getAttribute("src")).toBe("data:image/png;base64,AAAA");
  });

  test("updates activity, avatar, localized copy and receipt callbacks when their props change", async () => {
    const message: TextChatMessage = {
      id: "assistant", role: "assistant", text: "", localOnly: true,
      activity: { kind: "websearch", running: true, sites: ["example.com", "example.org"] },
    };
    const props = propsFor(message);
    const pending: ChatWorklogOperation = { messageId: message.id, requestId: "operation", receipt: null, error: null };
    const refreshed: ChatWorklogOperation[] = [];
    const undone: ChatWorklogOperation[] = [];
    const onRefresh = async (operation: ChatWorklogOperation) => { refreshed.push(operation); };
    const onUndo = async (operation: ChatWorklogOperation) => { undone.push(operation); };
    const { container, getByRole, rerender } = await render(<ChatMessageRow {...props} operations={[pending]} onRefresh={onRefresh} />);
    expect(container.querySelector(".chat-activity-websearch")?.getAttribute("role")).toBe("status");
    expect(container.querySelector(".chat-activity-websearch")?.getAttribute("aria-live")).toBe("polite");
    expect(container.querySelector(".chat-websearch-sites")?.getAttribute("title")).toBe("example.com · example.org");
    expect(container.querySelectorAll(".chat-websearch-sites-group")).toHaveLength(2);
    expect(container.querySelector(".chat-activity-running")).not.toBeNull();
    expect(container.textContent).toContain("本地回复");
    expect(container.querySelector(".chat-bubble")).toBeNull();
    fireEvent.click(getByRole("button", { name: "查询结果" }));
    expect(refreshed).toEqual([pending]);
    const committed: ChatWorklogOperation = {
      ...pending, undoable: true,
      receipt: { operationId: "operation", entityKind: "entry", entityId: "entry", revision: 1, businessDate: null, status: "committed" },
    };
    await rerender(<ChatMessageRow {...props} lang="en-US" t={dict("en-US")} resourceLanguage="en-US"
      message={{ ...message, activity: "工具已完成" }} avatarSources={["data:image/png;base64,BBBB"]}
      operations={[committed]} onUndo={onUndo} />);
    expect(container.querySelector(".chat-activity-websearch")).toBeNull();
    expect(container.textContent).toContain("工具已完成");
    expect(container.textContent).toContain("Local reply");
    expect(container.querySelector(".yume-avatar img")?.getAttribute("src")).toBe("data:image/png;base64,BBBB");
    fireEvent.click(getByRole("button", { name: "Undo" }));
    expect(undone).toEqual([committed]);
  });
});
