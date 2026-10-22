import { afterEach, describe, expect, jest, mock, test } from "bun:test";
import { Profiler } from "react";
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { dict } from "../lib/i18n";
import type { UpdateEvent, UpdateOutcome, UpdateStatus } from "../lib/updater";
import { UpdateFooter } from "./UpdateFooter";

afterEach(() => { cleanup(); jest.useRealTimers(); });
const t = dict("zh-CN");
const repo = "clxgame/deskmate";

function services(
  outcome: UpdateOutcome = { status: "upToDate", currentVersion: "0.4.3" },
  status: UpdateStatus = { status: "idle" },
) {
  return {
    getAppVersion: mock(async () => "0.4.3"),
    getWindowVisible: mock(async () => true),
    onWindowFocusChanged: mock(async (_listener: () => void): Promise<() => void> => () => undefined),
    checkUpdate: mock(async (_repo: string): Promise<string | null> => "0.4.4"),
    getUpdateStatus: mock(async (): Promise<UpdateStatus> => status),
    updateApp: mock(async (_repo: string, _onEvent: (event: UpdateEvent) => void) => outcome),
    cancelUpdate: mock(async (): Promise<void> => undefined),
  };
}

describe("conditional Windows and Mac update footer", () => {
  test("one click checks, downloads, verifies, installs and restarts without another action", async () => {
    const api = services({ status: "installed", version: "0.4.4" });
    api.updateApp.mockImplementation(async (_repo, onEvent) => {
      onEvent({ event: "checking" });
      onEvent({ event: "downloadStarted", data: { version: "0.4.4", contentLength: 100 } });
      onEvent({ event: "downloadProgress", data: {
        version: "0.4.4", downloaded: 100, contentLength: 100,
      } });
      onEvent({ event: "verifying", data: { version: "0.4.4" } });
      onEvent({ event: "installing", data: { version: "0.4.4" } });
      onEvent({ event: "restarting", data: { version: "0.4.4" } });
      return { status: "installed", version: "0.4.4" };
    });
    render(<UpdateFooter repo={repo} t={t} services={api} />);
    fireEvent.click(await screen.findByRole("button", { name: t.updateCheck }));
    await screen.findByText(t.updateRestarting);
    expect(api.updateApp).toHaveBeenCalledTimes(1);
    expect(api.updateApp.mock.calls[0]?.[0]).toBe(repo);
    expect(screen.getAllByRole("button")).toHaveLength(1);
  });

  test("reports current version without downloading or restarting", async () => {
    const api = services();
    render(<UpdateFooter repo={repo} t={t} services={api} />);
    fireEvent.click(await screen.findByRole("button", { name: t.updateCheck }));
    await waitFor(() => expect(screen.queryByRole("button")).toBeNull());
    expect(api.updateApp).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole("status")).toBeNull();
  });

  test("restores a backend update that is waiting for an active task", async () => {
    const api = services(undefined, { status: "waitingForIdle", version: "0.4.4" });
    render(<UpdateFooter repo={repo} t={t} services={api} />);
    await screen.findByText(t.updateWaitingForIdle);
    fireEvent.click(screen.getByRole("button", { name: t.updateCancelWait }));
    await waitFor(() => expect(api.cancelUpdate).toHaveBeenCalledTimes(1));
    expect(api.updateApp).not.toHaveBeenCalled();
  });

  test("prevents duplicate clicks while one update is pending", async () => {
    const api = services();
    let finish!: (outcome: UpdateOutcome) => void;
    api.updateApp.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
    render(<UpdateFooter repo={repo} t={t} services={api} />);
    const button = await screen.findByRole("button", { name: t.updateCheck });
    fireEvent.click(button);
    fireEvent.click(button);
    expect(api.updateApp).toHaveBeenCalledTimes(1);
    expect((button as HTMLButtonElement).disabled).toBe(true);
    finish({ status: "upToDate", currentVersion: "0.4.3" });
    await waitFor(() => expect(screen.queryByRole("button")).toBeNull());
  });
});


test("background check stays version-only while pending, then shows only the download button", async () => {
  const api = services();
  let resolve!: (version: string | null) => void;
  api.checkUpdate.mockImplementation(() => new Promise((done) => { resolve = done; }));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await screen.findByText("v0.4.3");
  expect(screen.queryByRole("button")).toBeNull();
  expect(screen.queryByRole("status")).toBeNull();
  await act(async () => resolve("0.4.4"));
  await screen.findByRole("button", { name: "下载更新" });
  expect(screen.queryByRole("status")).toBeNull();
  expect(screen.queryByText(t.updateHint)).toBeNull();
  expect(api.checkUpdate).toHaveBeenCalledWith(repo);
  expect(api.updateApp).not.toHaveBeenCalled();
});

for (const scenario of ["current", "offline", "missing repository"]) {
  test(`only version remains for ${scenario}`, async () => {
    const api = services();
    api.checkUpdate.mockImplementation(async () => {
      if (scenario === "offline") throw new Error("network");
      return null;
    });
    render(<UpdateFooter repo={scenario === "missing repository" ? "" : repo} t={t} services={api} />);
    await screen.findByText("v0.4.3");
    expect(screen.queryByRole("button")).toBeNull();
    expect(screen.queryByRole("status")).toBeNull();
    expect(screen.getByRole("contentinfo").textContent).toBe("v0.4.3");
    expect(api.updateApp).not.toHaveBeenCalled();
    expect(api.checkUpdate).toHaveBeenCalledTimes(scenario === "missing repository" ? 0 : 1);
  });
}

test("reconnecting retries a failed background check without starting a download", async () => {
  const api = services();
  api.checkUpdate.mockRejectedValueOnce(new Error("network"));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await screen.findByText("v0.4.3");
  fireEvent(window, new Event("online"));
  await screen.findByRole("button", { name: t.updateCheck });
  expect(api.checkUpdate).toHaveBeenCalledTimes(2);
  expect(api.updateApp).not.toHaveBeenCalled();
});

test("a stale repository response cannot reveal the old repository update", async () => {
  const api = services();
  let resolve!: (version: string | null) => void;
  api.checkUpdate.mockImplementation((value) => value === repo
    ? new Promise((done) => { resolve = done; }) : Promise.resolve(null));
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await waitFor(() => expect(api.checkUpdate).toHaveBeenCalledWith(repo));
  view.rerender(<UpdateFooter repo="example/other" t={t} services={api} />);
  await act(async () => resolve("0.4.4"));
  await screen.findByText("v0.4.3");
  expect(screen.queryByRole("button")).toBeNull();
  expect(api.checkUpdate).toHaveBeenCalledWith("example/other");
});

test("download failure keeps an explicit retry available", async () => {
  const api = services();
  api.updateApp.mockRejectedValueOnce(new Error("network"));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  fireEvent.click(await screen.findByRole("button", { name: t.updateCheck }));
  await screen.findByRole("status");
  const button = screen.getByRole("button", { name: t.updateCheck });
  expect((button as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(button);
  await waitFor(() => expect(api.updateApp).toHaveBeenCalledTimes(2));
  await waitFor(() => expect(screen.queryByRole("button")).toBeNull());
});

test("an idle status requested before a click cannot overwrite download progress", async () => {
  const api = services();
  let resolveStatus!: (status: UpdateStatus) => void;
  api.getUpdateStatus.mockImplementation(() => new Promise((done) => { resolveStatus = done; }));
  api.updateApp.mockImplementation(async (_repo, onEvent) => {
    onEvent({ event: "downloadProgress", data: { version: "0.4.4", downloaded: 50, contentLength: 100 } });
    return new Promise(() => {});
  });
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  fireEvent.click(await screen.findByRole("button", { name: t.updateCheck }));
  await act(async () => resolveStatus({ status: "idle" }));
  expect(screen.getByRole("status").textContent).toContain("50%");
});


async function flush(): Promise<void> {
  await act(async () => { for (let index = 0; index < 12; index += 1) await Promise.resolve(); });
}
async function advance(milliseconds: number): Promise<void> {
  await act(async () => { jest.advanceTimersByTime(milliseconds); });
  await flush();
}

test("idle native-hidden Settings has no 500 ms status or visibility polling", async () => {
  jest.useFakeTimers();
  const api = services();
  api.getWindowVisible.mockResolvedValue(false);
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  expect(api.checkUpdate).not.toHaveBeenCalled();
  for (let index = 0; index < 29; index += 1) await advance(1000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  expect(api.getWindowVisible).toHaveBeenCalledTimes(1);
  await advance(1000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
});

test("slow status IPC stays single-flight during focus and timer storms", async () => {
  jest.useFakeTimers();
  const api = services();
  let finish!: (value: UpdateStatus) => void;
  api.getUpdateStatus.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  for (let index = 0; index < 12; index += 1) {
    fireEvent(window, new Event("focus"));
    fireEvent(window, new Event("online"));
    await advance(1000);
  }
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  finish({ status: "idle" });
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
});

test("native reopen refreshes immediately even if document visibility never changed", async () => {
  jest.useFakeTimers();
  const api = services();
  api.getWindowVisible.mockResolvedValue(false);
  let nativeFocus!: () => void;
  const stop = mock(() => undefined);
  api.onWindowFocusChanged.mockImplementation(async (listener) => { nativeFocus = listener; return stop; });
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  api.getWindowVisible.mockResolvedValue(true);
  api.getUpdateStatus.mockResolvedValue({ status: "waitingForIdle", version: "0.4.4" });
  await act(async () => nativeFocus());
  await flush();
  expect(screen.getByRole("status").textContent).toBe(t.updateWaitingForIdle);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
  await advance(1000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(3);
  view.unmount();
  await flush();
  expect(stop).toHaveBeenCalledTimes(1);
  await advance(30000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(3);
});

test("a later external update is discovered by idle fallback and slows down on hide", async () => {
  jest.useFakeTimers();
  const api = services();
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  api.getUpdateStatus.mockResolvedValue({ status: "downloading", version: "0.4.4", downloaded: 50, contentLength: 100 });
  await advance(4999);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  await advance(1);
  expect(screen.getByRole("status").textContent).toContain("50%");
  api.getWindowVisible.mockResolvedValue(false);
  fireEvent(window, new Event("blur"));
  await flush();
  const count = api.getUpdateStatus.mock.calls.length;
  await advance(4999);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(count);
  await advance(1);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(count + 1);
});

test("status read failure retries and a visibility event refreshes immediately", async () => {
  jest.useFakeTimers();
  const api = services();
  api.getUpdateStatus.mockRejectedValueOnce(new Error("temporarily unavailable"));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  api.getUpdateStatus.mockResolvedValue({ status: "verifying", version: "0.4.4" });
  fireEvent(document, new Event("visibilitychange"));
  await flush();
  expect(screen.getByRole("status").textContent).toBe(t.updateVerifying);
});

test("replaced services cannot publish a late status or overlap the existing request", async () => {
  jest.useFakeTimers();
  const oldApi = services();
  let finish!: (value: UpdateStatus) => void;
  oldApi.getUpdateStatus.mockImplementation(() => new Promise((resolve) => { finish = resolve; }));
  const view = render(<UpdateFooter repo={repo} t={t} services={oldApi} />);
  await flush();
  const nextApi = services();
  view.rerender(<UpdateFooter repo={repo} t={t} services={nextApi} />);
  await flush();
  expect(nextApi.getUpdateStatus).not.toHaveBeenCalled();
  finish({ status: "installing", version: "old" });
  await flush();
  expect(nextApi.getUpdateStatus).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("status")).toBeNull();
});


test("locale changes reject the old snapshot and reuse its in-flight gate", async () => {
  jest.useFakeTimers();
  const api = services();
  let finish!: (value: UpdateStatus) => void;
  api.getUpdateStatus.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  view.rerender(<UpdateFooter repo={repo} t={dict("en-US")} services={api} />);
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  finish({ status: "error", code: "network" });
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
  expect(screen.queryByRole("status")).toBeNull();
});

test("unmount rejects late status and listener registration without restarting polling", async () => {
  jest.useFakeTimers();
  const api = services();
  let finish!: (value: UpdateStatus) => void;
  let register!: (stop: () => void) => void;
  const stop = mock(() => undefined);
  api.getUpdateStatus.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  api.onWindowFocusChanged.mockImplementationOnce(() => new Promise<() => void>((resolve) => { register = resolve; }));
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  fireEvent(window, new Event("focus"));
  view.unmount();
  finish({ status: "installing", version: "late" });
  register(stop);
  await flush();
  await advance(60000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  expect(api.getWindowVisible).toHaveBeenCalledTimes(1);
  expect(stop).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("status")).toBeNull();
});

test("own update Channel carries progress without competing status reads and permits retry", async () => {
  jest.useFakeTimers();
  const api = services();
  let report!: (event: UpdateEvent) => void;
  let fail!: (error: Error) => void;
  api.updateApp.mockImplementationOnce((_repo, onEvent) => {
    report = onEvent;
    return new Promise((_resolve, reject) => { fail = reject; });
  });
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  await act(async () => report({ event: "downloadProgress", data: { version: "v", downloaded: 50, contentLength: 100 } }));
  for (let index = 0; index < 10; index += 1) await advance(1000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  expect(screen.getByRole("status").textContent).toContain("50%");
  fail(new Error("network"));
  await flush();
  expect((screen.getByRole("button", { name: t.updateCheck }) as HTMLButtonElement).disabled).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  await flush();
  expect(api.updateApp).toHaveBeenCalledTimes(2);
});

test("in-progress rejection joins the same status gate then restores remote progress", async () => {
  jest.useFakeTimers();
  const api = services();
  let finish!: (value: UpdateStatus) => void;
  api.getUpdateStatus.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  api.updateApp.mockRejectedValueOnce(new Error("in_progress"));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
  api.getUpdateStatus.mockResolvedValue({ status: "waitingForIdle", version: "v" });
  finish({ status: "idle" });
  await flush();
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
  expect(screen.getByRole("status").textContent).toBe(t.updateWaitingForIdle);
});


test("changing locale keeps the live update Channel and its cancel-wait action", async () => {
  jest.useFakeTimers();
  const api = services();
  let report!: (event: UpdateEvent) => void;
  api.updateApp.mockImplementation((_repo, onEvent) => {
    report = onEvent;
    return new Promise(() => undefined);
  });
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  const en = dict("en-US");
  view.rerender(<UpdateFooter repo={repo} t={en} services={api} />);
  await act(async () => report({ event: "waitingForIdle", data: { version: "v" } }));
  await flush();
  expect(screen.getByRole("status").textContent).toBe(en.updateWaitingForIdle);
  fireEvent.click(screen.getByRole("button", { name: en.updateCancelWait }));
  await flush();
  expect(api.cancelUpdate).toHaveBeenCalledTimes(1);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(1);
});


test("slow native visibility IPC also shares the single-flight event gate", async () => {
  jest.useFakeTimers();
  const api = services();
  let finish!: (value: boolean) => void;
  api.getWindowVisible.mockImplementationOnce(() => new Promise((resolve) => { finish = resolve; }));
  render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  for (let index = 0; index < 5; index += 1) {
    fireEvent(window, new Event("focus"));
    fireEvent(document, new Event("visibilitychange"));
    await advance(1000);
  }
  expect(api.getWindowVisible).toHaveBeenCalledTimes(1);
  expect(api.getUpdateStatus).not.toHaveBeenCalled();
  finish(true);
  await flush();
  expect(api.getWindowVisible).toHaveBeenCalledTimes(2);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(2);
});

test("replacing services during an update restores the host state and rejects the old Channel", async () => {
  jest.useFakeTimers();
  const oldApi = services();
  let report!: (event: UpdateEvent) => void;
  let complete!: (outcome: UpdateOutcome) => void;
  oldApi.updateApp.mockImplementation((_repo, onEvent) => {
    report = onEvent;
    return new Promise((resolve) => { complete = resolve; });
  });
  const view = render(<UpdateFooter repo={repo} t={t} services={oldApi} />);
  await flush();
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  const nextApi = services(undefined, { status: "waitingForIdle", version: "new" });
  view.rerender(<UpdateFooter repo={repo} t={t} services={nextApi} />);
  await flush();
  expect(screen.getByRole("status").textContent).toBe(t.updateWaitingForIdle);
  await act(async () => {
    report({ event: "installing", data: { version: "old" } });
    complete({ status: "installed", version: "old" });
  });
  await flush();
  expect(screen.getByRole("status").textContent).toBe(t.updateWaitingForIdle);
  expect(nextApi.getUpdateStatus).toHaveBeenCalledTimes(1);
  fireEvent.click(screen.getByRole("button", { name: t.updateCancelWait }));
  await flush();
  expect(nextApi.cancelUpdate).toHaveBeenCalledTimes(1);
  expect(oldApi.cancelUpdate).not.toHaveBeenCalled();
});

test("unchanged status snapshots do not commit another footer render", async () => {
  jest.useFakeTimers();
  const api = services(undefined, { status: "waitingForIdle", version: "v" });
  const commits = mock(() => undefined);
  render(<Profiler id="footer" onRender={commits}><UpdateFooter repo={repo} t={t} services={api} /></Profiler>);
  await flush();
  const before = commits.mock.calls.length;
  for (let index = 0; index < 10; index += 1) await advance(1000);
  expect(api.getUpdateStatus).toHaveBeenCalledTimes(11);
  expect(commits).toHaveBeenCalledTimes(before);
});


test("an active update failure uses the locale committed after the update started", async () => {
  jest.useFakeTimers();
  const api = services();
  let fail!: (error: Error) => void;
  api.updateApp.mockImplementationOnce(() => new Promise((_resolve, reject) => { fail = reject; }));
  const view = render(<UpdateFooter repo={repo} t={t} services={api} />);
  await flush();
  fireEvent.click(screen.getByRole("button", { name: t.updateCheck }));
  const en = dict("en-US");
  view.rerender(<UpdateFooter repo={repo} t={en} services={api} />);
  await flush();
  await act(async () => fail(new Error("network")));
  await flush();
  expect(screen.getByRole("status").textContent).toBe(en.updateNetworkError);
  expect(screen.getByRole("status").textContent).not.toBe(t.updateNetworkError);
  expect((screen.getByRole("button", { name: en.updateCheck }) as HTMLButtonElement).disabled).toBe(false);
});
