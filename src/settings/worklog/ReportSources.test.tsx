import { afterEach, expect, mock, test } from "bun:test";
import * as core from "@tauri-apps/api/core";
import * as events from "@tauri-apps/api/event";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import type { ReportSourcePage, ReportVersionContent } from "../../lib/worklog";
import { worklogLabels } from "./worklogLabels";
let changed = () => {};
const invoke = mock<(command: string, args?: unknown) => Promise<unknown>>(() => Promise.resolve({ sources: [], nextOffset: null }));
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
mock.module("@tauri-apps/api/event", () => ({ ...events, listen: mock(async (_name: string, callback: () => void) => { changed = callback; return () => {}; }) }));
const { ReportSources } = await import("./ReportSources");
const version: ReportVersionContent = { id: "v1", reportId: "r1", version: 1, bodyMarkdown: "body", coverageDates: [], sourceCount: 123, origin: "manual", generatedAt: "now", modelId: null };
const page = (prefix: string, nextOffset: number | null): ReportSourcePage => ({
  sources: Array.from({ length: 50 }, (_, n) => ({ source: { kind: "entry", id: prefix + n, revision: 1 }, text: prefix + n, businessDate: "2026-09-07", project: null, entryStatus: null })), nextOffset,
});
afterEach(() => { cleanup(); invoke.mockReset(); changed = () => {}; });
function toggle(element: HTMLDetailsElement, open: boolean) { act(() => { element.open = open; fireEvent(element, new Event("toggle")); }); }
test("collapsed sources do no IPC; one page at a time is mounted and closing releases it", async () => {
  invoke.mockImplementation((_command, args) => Promise.resolve(page((args as { offset: number }).offset === 0 ? "first" : "second", (args as { offset: number }).offset === 0 ? 50 : null)));
  const { container } = render(<ReportSources version={version} labels={worklogLabels("zh-CN")} missing={[]} />);
  expect(invoke).not.toHaveBeenCalled();
  const details = container.querySelector("details")!;
  toggle(details, true);
  await screen.findByText("first0");
  expect(container.querySelectorAll("li").length).toBe(50);
  fireEvent.click(screen.getByRole("button", { name: /→$/ }));
  await screen.findByText("second0");
  expect(screen.queryByText("first0")).toBeNull();
  expect(container.querySelectorAll("li").length).toBe(50);
  toggle(details, false);
  expect(container.querySelectorAll("li").length).toBe(0);
});
test("forget notification invalidates an in-flight response before it can republish old sources", async () => {
  const pending = Promise.withResolvers<ReportSourcePage>();
  invoke.mockImplementationOnce(() => pending.promise).mockImplementation(() => Promise.resolve({ sources: [], nextOffset: null }));
  const { container } = render(<ReportSources version={version} labels={worklogLabels("zh-CN")} missing={[]} />);
  toggle(container.querySelector("details")!, true);
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(1));
  await act(async () => { changed(); pending.resolve(page("forgotten", null)); await Promise.resolve(); });
  await waitFor(() => expect(invoke).toHaveBeenCalledTimes(2));
  expect(screen.queryByText("forgotten0")).toBeNull();
});
