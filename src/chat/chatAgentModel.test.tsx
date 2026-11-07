import { expect, test } from "bun:test";
import { act, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { nativeHistoryFixture } from "../testing/historyCatalogFixtures";
import { agentRun, ChatApp, invoke, promptRequests, setAgentProjection, setHistory, registerChatAttachmentHarness, deferModelSelection, finishModelSelection } from "./chatAttachmentSendHarness.test";

registerChatAttachmentHarness();

test("an unrelated active run permits model selection but still prevents a second task", async () => {
  setAgentProjection(agentRun);
  render(<ChatApp />);
  await screen.findByRole("button", { name: "返回任务" });
  fireEvent.click(await screen.findByRole("button", { name: "Claude Sonnet 4.5" }));
  fireEvent.click(await screen.findByRole("menuitemradio", { name: /Selected Model B/ }));
  await screen.findByRole("button", { name: "Selected Model B" });
  expect(invoke.mock.calls.some(([command]) => command === "history_model_selection_set")).toBe(true);
  const input = screen.getByPlaceholderText("输入消息,Enter 发送");
  fireEvent.change(input, { target: { value: "new draft" } });
  await act(async () => { fireEvent.keyDown(input, { key: "Enter" }); });
  expect((screen.getByRole("button", { name: "发送" }) as HTMLButtonElement).disabled).toBe(true);
  expect(promptRequests).toHaveLength(0);
  expect(invoke.mock.calls.some(([command]) => command === "agent_run_start")).toBe(false);
});

test("a pending model choice cannot race Enter into sending with the old model", async () => {
  render(<ChatApp />);
  fireEvent.click(await screen.findByRole("button", { name: "Claude Sonnet 4.5" }));
  deferModelSelection();
  fireEvent.click(await screen.findByRole("menuitemradio", { name: /Selected Model B/ }));
  const input = screen.getByPlaceholderText("输入消息,Enter 发送");
  fireEvent.change(input, { target: { value: "use new model" } });
  await act(async () => { fireEvent.keyDown(input, { key: "Enter" }); });
  expect(promptRequests).toHaveLength(0);
  expect((screen.getByRole("button", { name: "发送" }) as HTMLButtonElement).disabled).toBe(true);
  await act(async () => { finishModelSelection(); });
  await screen.findByRole("button", { name: "Selected Model B" });
  fireEvent.keyDown(input, { key: "Enter" });
  await waitFor(() => expect(promptRequests).toHaveLength(1));
  expect(promptRequests[0].model?.modelID).toBe("selected-model-b");
});

for (const workspacePath of ["C:\\workspace", "\\\\?\\C:\\WORKSPACE\\"]) test(`return and Stop use host identity for ${workspacePath}`, async () => {
  setAgentProjection({ ...agentRun, workspacePath });
  setHistory({ [agentRun.sessionId]: {
    id: agentRun.sessionId, title: "Active task", created: 1, updated: 1, originRunId: agentRun.runId,
    messages: [{ role: "user", text: "task input", time: 1 }],
  } });
  render(<ChatApp />);
  fireEvent.click(await screen.findByRole("button", { name: "返回任务" }));
  await screen.findByText("task input");
  const stop = await screen.findByRole("button", { name: "停" });
  expect(screen.queryByRole("button", { name: "返回任务" })).toBeNull();
  fireEvent.click(document.querySelector(".composer-model button")!);
  await screen.findByText("请先完成当前回复再切换模型");
  expect(screen.queryByRole("menuitemradio", { name: /Selected Model B/ })).toBeNull();
  fireEvent.click(stop);
  await waitFor(() => expect(invoke.mock.calls.some(([command]) => command === "agent_run_cancel")).toBe(true));
});

test("the same session ID in a different directory cannot acquire this task's Stop control", async () => {
  setAgentProjection(agentRun, nativeHistoryFixture(agentRun.sessionId, "C:/other").key);
  setHistory({ [agentRun.sessionId]: {
    id: agentRun.sessionId, title: "Same bare id", created: 1, updated: 1, originRunId: agentRun.runId,
    messages: [{ role: "user", text: "different directory", time: 1 }],
  } });
  render(<ChatApp />);
  fireEvent.click(await screen.findByRole("button", { name: "返回任务" }));
  await screen.findByText("different directory");
  expect(screen.queryByRole("button", { name: "停" })).toBeNull();
  expect(screen.getByRole("button", { name: "返回任务" })).toBeTruthy();
});
