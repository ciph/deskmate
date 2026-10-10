import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { cleanup, fireEvent, render, waitFor } from "@testing-library/react";
import * as core from "@tauri-apps/api/core";
import { restoreTauriModuleFixture } from "../testing/tauriModuleFixture";
import type { WindowsComponentStatus } from "../lib/windowsComponent";

let status: WindowsComponentStatus;
const invoke = mock<(command: string, args?: unknown) => Promise<unknown>>(async (command) => {
  if (command === "windows_component_status") return status;
});
mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
const { WindowsComponentControl } = await import("./WindowsComponentControl");
beforeEach(() => {
  mock.module("@tauri-apps/api/core", () => ({ ...core, invoke }));
  status = { phase: "missing", downloaded: 0, total: 54160658, version: "1.3.24", supported: true, activation: "idle", error: null };
  invoke.mockReset();
  invoke.mockImplementation(async (command) => {
    if (command === "windows_component_status") return { ...status };
    if (command === "windows_component_install") status = { ...status, phase: "downloading" };
    if (command === "windows_component_cancel") status = { ...status, phase: "missing" };
  });
});
afterEach(() => { cleanup(); restoreTauriModuleFixture(); });
function control(enabled = false) {
  return render(<WindowsComponentControl enabled={enabled} language="en-US" label="Windows UI control" />);
}
test("first enable shows the download size and does not download before confirmation", async () => {
  const ui = control();
  await ui.findByText(/First use downloads 51.7 MB/);
  expect(invoke.mock.calls.some(([cmd]) => cmd === "windows_component_install")).toBe(false);
  fireEvent.click(ui.getByRole("checkbox"));
  fireEvent.click(ui.getByRole("button", { name: "Cancel" }));
  expect(invoke.mock.calls.some(([cmd]) => cmd === "windows_component_install")).toBe(false);
  fireEvent.click(ui.getByRole("checkbox"));
  fireEvent.click(ui.getByRole("button", { name: "Download and enable" }));
  await ui.findByRole("progressbar");
  expect(invoke.mock.calls.filter(([cmd]) => cmd === "windows_component_install")).toHaveLength(1);
  expect(ui.getByRole("checkbox")).toHaveProperty("disabled", true);
});
test("cancelled download returns to a retryable state", async () => {
  status = { ...status, phase: "downloading", downloaded: 1000000 };
  const ui = control();
  fireEvent.click(await ui.findByRole("button", { name: "Cancel download" }));
  await ui.findByRole("button", { name: "Download and enable" });
  expect(invoke.mock.calls.some(([cmd]) => cmd === "windows_component_cancel")).toBe(true);
  expect(ui.queryByRole("progressbar")).toBeNull();
});
test("a cached component enables without downloading and explains deferred activation", async () => {
  status = { ...status, phase: "ready" };
  const ui = control();
  await ui.findByText(/Component installed/);
  fireEvent.click(ui.getByRole("checkbox"));
  expect(ui.getByText(/takes effect after current tasks finish/)).toBeDefined();
  fireEvent.click(ui.getByRole("button", { name: "Apply change" }));
  await waitFor(() => expect(invoke.mock.calls).toContainEqual(["windows_component_set_enabled", { enabled: true }]));
  expect(invoke.mock.calls.some(([cmd]) => cmd === "windows_component_install")).toBe(false);
});
test("an upgraded enabled preference with a missing component offers repair", async () => {
  const ui = control(true);
  await ui.findByText(/needs to be downloaded/);
  expect(ui.getByRole("checkbox")).toHaveProperty("checked", true);
  fireEvent.click(ui.getByRole("button", { name: "Download and enable" }));
  fireEvent.click(ui.getByRole("button", { name: "Download and enable" }));
  await ui.findByRole("progressbar");
});
test("checksum failure is visible and allows a new download", async () => {
  status = { ...status, phase: "error", error: "component_checksum" };
  const ui = control();
  expect((await ui.findByRole("alert")).textContent).toContain("was not installed");
  fireEvent.click(ui.getByRole("button", { name: "Download and enable" }));
  fireEvent.click(ui.getByRole("button", { name: "Download and enable" }));
  await ui.findByRole("progressbar");
});
test("waiting activation is visible and can be superseded by disabling", async () => {
  status = { ...status, phase: "ready", activation: "waiting" };
  const ui = control(true);
  await ui.findByText(/Waiting for tasks/);
  fireEvent.click(ui.getByRole("checkbox"));
  fireEvent.click(ui.getByRole("button", { name: "Apply change" }));
  await waitFor(() => expect(invoke.mock.calls).toContainEqual(["windows_component_set_enabled", { enabled: false }]));
});
test("unsupported platforms cannot request the Windows download", async () => {
  status = { ...status, supported: false };
  const ui = control();
  await ui.findByText("Available on Windows x64 only.");
  expect(ui.getByRole("checkbox")).toHaveProperty("disabled", true);
  expect(ui.queryByRole("button", { name: "Download and enable" })).toBeNull();
});
test("closing and reopening the tab observes the existing download", async () => {
  status = { ...status, phase: "downloading", downloaded: 123456 };
  const first = control();
  await first.findByRole("progressbar");
  first.unmount();
  const reopened = control();
  expect(await reopened.findByRole("progressbar")).toHaveProperty("value", 123456);
  expect(invoke.mock.calls.some(([cmd]) => cmd === "windows_component_install")).toBe(false);
});
