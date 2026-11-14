import { afterEach, expect, jest, mock, test } from "bun:test";
import * as core from "@tauri-apps/api/core";
import { act, cleanup, render } from "@testing-library/react";
import { dict } from "../lib/i18n";
const invoke = mock(() => Promise.resolve({ kind: "local", localAvailable: true, todayTokens: 1, todayRequests: 1, topModels: [] }));
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
const { AiUsage } = await import("./AiUsage");
const { SettingsVisibilityContext } = await import("./settingsVisibility");
afterEach(() => { cleanup(); jest.useRealTimers(); });

// Separate process from real-clock usage tests: Bun/Happy DOM timer restoration can interfere with waitFor.
test("hidden cards stop usage queries and reveal reloads without waiting five minutes", async () => {
  jest.useFakeTimers();
  const view = (visible: boolean) => <SettingsVisibilityContext.Provider value={visible}><AiUsage enabled providerId="local" label="Local" index={0} t={dict("zh-CN")} /></SettingsVisibilityContext.Provider>;
  const rendered = render(view(false)); await act(async () => { jest.advanceTimersByTime(600_000); }); expect(invoke).not.toHaveBeenCalled();
  rendered.rerender(view(true)); await act(async () => {}); expect(invoke).toHaveBeenCalledTimes(1);
  rendered.rerender(view(false)); await act(async () => { jest.advanceTimersByTime(600_000); }); expect(invoke).toHaveBeenCalledTimes(1);
  rendered.rerender(view(true)); await act(async () => {}); expect(invoke).toHaveBeenCalledTimes(2);
});
