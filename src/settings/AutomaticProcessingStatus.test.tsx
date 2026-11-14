import { afterEach, expect, jest, mock, test } from "bun:test";
import * as core from "@tauri-apps/api/core";
import { act, cleanup, fireEvent, render, screen } from "@testing-library/react";
import { dict } from "../lib/i18n";
const invoke = mock<(cmd: string) => Promise<unknown>>(() => Promise.resolve({ pending: 1, failed: 1 }));
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
const { AutomaticProcessingStatus } = await import("./AutomaticProcessingStatus");
const { SettingsVisibilityContext } = await import("./settingsVisibility");
const t = dict("zh-CN");
afterEach(() => { cleanup(); jest.useRealTimers(); });

test("native visibility pauses status reads while retry remains functional", async () => {
  jest.useFakeTimers(); const view = (visible: boolean) => <SettingsVisibilityContext.Provider value={visible}><AutomaticProcessingStatus t={t} /></SettingsVisibilityContext.Provider>;
  const rendered = render(view(false)); await act(async () => { jest.advanceTimersByTime(60_000); }); expect(invoke).not.toHaveBeenCalled();
  rendered.rerender(view(true)); await act(async () => {}); expect(invoke).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("status")).toBeTruthy();
  await act(async () => { fireEvent.click(screen.getByRole("button", { name: t.automaticProcessingRetry })); });
  expect(invoke.mock.calls.map(([cmd]) => cmd)).toEqual(["memory_automation_status", "memory_automation_retry", "memory_automation_status"]);
  rendered.rerender(view(false)); await act(async () => { jest.advanceTimersByTime(60_000); }); expect(invoke).toHaveBeenCalledTimes(3);
  rendered.rerender(view(true)); await act(async () => {}); expect(invoke).toHaveBeenCalledTimes(4);
});
