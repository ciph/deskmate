import { afterEach, beforeEach, describe, expect, test } from "bun:test";
import { screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { containsProviderApiKey } from "./providerKeyGuard";
import {
  cleanupChatHarness, completedTool, invoke, receiveChatEvent,
  renderChat, resetChatHarness, wasPromptSent,
} from "../testing/providerConnectionHarness";

beforeEach(resetChatHarness);
afterEach(cleanupChatHarness);

describe("Chat after external deployment retirement", () => {
  test("keeps the existing key detection boundary", () => {
    expect(containsProviderApiKey("opencode api_key=syntheticsecret123")).toBe(true);
    expect(containsProviderApiKey("CC Switch bearer syntheticsecret123")).toBe(true);
    expect(containsProviderApiKey("How does OpenCode work?")).toBe(false);
  });

  test("blocks a pasted key before sending and directs the user to AI settings", async () => {
    await renderChat();
    const user = userEvent.setup();
    const secret = "sk-synthetic-retirement-canary";
    const input = screen.getByPlaceholderText("输入消息,Enter 发送");
    await user.click(input);
    await user.paste(`opencode API Key: ${secret}`);
    await user.keyboard("{Enter}");
    await screen.findByText("检测到疑似 API Key，消息未发送。请前往设置 → AI 填写凭据。");
    expect(wasPromptSent()).toBe(false);
    expect(document.body.innerHTML).not.toContain(secret);
    expect(invoke.mock.calls.some(([command]) => command.includes("ccswitch"))).toBe(false);
  });

  test("old tool results remain ordinary history and cannot reopen setup", async () => {
    await renderChat();
    receiveChatEvent({ type: "message.part.updated", properties: {
      part: completedTool(JSON.stringify({ version: 1, kind: "opencode_provider_draft", providerName: "old" })),
    } });
    await waitFor(() => expect(invoke.mock.calls.some(([command]) => command.includes("ccswitch"))).toBe(false));
    expect(screen.queryByLabelText("API Key")).toBeNull();
    expect(screen.queryByText("安全配置 OpenCode")).toBeNull();
  });
});
