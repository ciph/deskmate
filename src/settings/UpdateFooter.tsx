import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { isTauri } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { getAppVersion } from "../lib/settings";
import {
  cancelUpdate,
  checkUpdate,
  getUpdateStatus,
  updateApp,
  type UpdateEvent,
  type UpdateStatus,
} from "../lib/updater";
import type { Dict } from "../lib/i18n";
import { updateErrorMessage } from "./updateErrors";
import { initialUpdateState, reduceUpdateState, type UpdateAction } from "./updateState";

const defaultServices = {
  getAppVersion, checkUpdate, updateApp, getUpdateStatus, cancelUpdate,
  getWindowVisible: async (): Promise<boolean> => isTauri() ? getCurrentWindow().isVisible() : !document.hidden,
  onWindowFocusChanged: async (listener: () => void): Promise<() => void> => isTauri()
    ? getCurrentWindow().onFocusChanged(listener) : () => undefined,
};
const CHECK_INTERVAL_MS = 15 * 60 * 1000;
const FOCUS_CHECK_INTERVAL_MS = 60 * 1000;
const busyStatuses = ["checking", "downloading", "verifying", "waitingForIdle", "installing", "restarting"];

export function UpdateFooter({ repo, t, services = defaultServices }: {
  readonly repo: string;
  readonly t: Dict;
  readonly services?: typeof defaultServices;
}) {
  const [version, setVersion] = useState("");
  const [state, setState] = useState(initialUpdateState);
  const stateRef = useRef(initialUpdateState);
  const [available, setAvailable] = useState<{ repo: string; version: string } | null>(null);
  const [attempted, setAttempted] = useState(false);
  const updateInFlight = useRef(false);
  const backendBusy = useRef(false);
  const attempt = useRef(0);
  const lifecycle = useRef(0);
  const latestDict = useRef(t);
  // Publish the committed locale before IPC callbacks resume; abandoned renders must not change it.
  useLayoutEffect(() => { latestDict.current = t; }, [t]);
  const visible = useRef(false);
  const probeRunning = useRef(false);
  const pendingProbe = useRef<{ checkManifest: boolean; forceManifest: boolean } | null>(null);
  const refreshStatus = useRef<(checkManifest?: boolean, forceManifest?: boolean) => void>(() => undefined);
  const scheduleStatus = useRef<() => void>(() => undefined);
  const checkManifest = useRef<(force: boolean) => void>(() => undefined);

  const dispatch = (action: UpdateAction): void => {
    const next = reduceUpdateState(stateRef.current, action);
    if (next === stateRef.current) return;
    stateRef.current = next;
    setState(next);
  };

  const applyStatus = (status: UpdateStatus): void => {
    const wasBusy = backendBusy.current;
    backendBusy.current = busyStatuses.includes(status.status);
    if (backendBusy.current && !wasBusy) setAttempted(true);
    switch (status.status) {
      case "idle": dispatch({ type: "idle" }); return;
      case "checking": dispatch({ type: "check" }); return;
      case "downloading":
        dispatch({ type: "download", version: status.version, downloaded: status.downloaded,
          contentLength: status.contentLength }); return;
      case "verifying": dispatch({ type: "verify", version: status.version }); return;
      case "waitingForIdle": dispatch({ type: "wait", version: status.version }); return;
      case "installing": dispatch({ type: "install", version: status.version }); return;
      case "restarting": dispatch({ type: "restart", version: status.version }); return;
      case "upToDate": dispatch({ type: "uptodate" }); return;
      case "error": dispatch({ type: "fail", message: updateErrorMessage(t, status.code) }); return;
      default: {
        const exhaustive: never = status;
        return exhaustive;
      }
    }
  };

  useEffect(() => {
    lifecycle.current += 1;
    updateInFlight.current = false;
    backendBusy.current = false;
    return () => { lifecycle.current += 1; };
  }, [services]);

  useEffect(() => {
    let disposed = false;
    let timer: number | undefined;
    void services.getAppVersion().then((loaded) => { if (!disposed) setVersion(loaded); }, () => undefined);
    const schedule = (): void => {
      window.clearTimeout(timer);
      const delay = visible.current
        ? (backendBusy.current ? 1000 : 5000)
        : (backendBusy.current ? 5000 : 30000);
      timer = window.setTimeout(() => refresh(), delay);
    };
    const refresh = (shouldCheckManifest = false, forceManifest = false): void => {
      if (disposed) return;
      window.clearTimeout(timer);
      if (probeRunning.current) {
        pendingProbe.current = {
          checkManifest: shouldCheckManifest || Boolean(pendingProbe.current?.checkManifest),
          forceManifest: forceManifest || Boolean(pendingProbe.current?.forceManifest),
        };
        return;
      }
      probeRunning.current = true;
      const requestedAttempt = attempt.current;
      void (async () => {
        // Native hide does not reliably change document.hidden in every webview.
        const nativeVisible = await services.getWindowVisible().catch(() => !document.hidden);
        if (disposed) return;
        const nextVisible = nativeVisible && !document.hidden;
        const becameVisible = !visible.current && nextVisible;
        visible.current = nextVisible;
        if (nextVisible && (shouldCheckManifest || becameVisible)) checkManifest.current(forceManifest);
        // This window already receives the active operation's progress Channel.
        if (updateInFlight.current) return;
        const status = await services.getUpdateStatus();
        // Ignore stale hosts/locale effects, pre-click reads and missing payloads.
        if (!disposed && status && !updateInFlight.current && requestedAttempt === attempt.current) applyStatus(status);
      })().catch(() => undefined).finally(() => {
        probeRunning.current = false;
        const pending = pendingProbe.current;
        pendingProbe.current = null;
        if (pending) refreshStatus.current(pending.checkManifest, pending.forceManifest);
        else if (!disposed) schedule();
      });
    };
    refreshStatus.current = refresh;
    scheduleStatus.current = schedule;
    const onVisibility = (): void => refresh(true);
    const onOnline = (): void => refresh(true, true);
    const unlisten = services.onWindowFocusChanged(onVisibility);
    void unlisten.catch(() => undefined);
    window.addEventListener("focus", onVisibility);
    window.addEventListener("blur", onVisibility);
    window.addEventListener("online", onOnline);
    document.addEventListener("visibilitychange", onVisibility);
    refresh(true, true);
    return () => {
      disposed = true;
      window.clearTimeout(timer);
      window.removeEventListener("focus", onVisibility);
      window.removeEventListener("blur", onVisibility);
      window.removeEventListener("online", onOnline);
      document.removeEventListener("visibilitychange", onVisibility);
      void unlisten.then((stop) => stop()).catch(() => undefined);
    };
  }, [services, t]);

  useEffect(() => {
    let disposed = false;
    let checking = false;
    let lastCheck = 0;
    setAvailable(null);
    const check = async (force = false): Promise<void> => {
      if (!repo.trim() || checking || updateInFlight.current || backendBusy.current || !visible.current || document.hidden) return;
      if (!force && Date.now() - lastCheck < FOCUS_CHECK_INTERVAL_MS) return;
      checking = true;
      lastCheck = Date.now();
      const requestedAttempt = attempt.current;
      try {
        const nextVersion = await services.checkUpdate(repo);
        if (!disposed && requestedAttempt === attempt.current) {
          setAvailable((current) => current?.repo === repo && current.version === nextVersion
            ? current : nextVersion ? { repo, version: nextVersion } : null);
        }
      } catch {
        if (!disposed && requestedAttempt === attempt.current) setAvailable(null);
      } finally {
        checking = false;
      }
    };
    checkManifest.current = (force) => { void check(force); };
    void check(true);
    const timer = window.setInterval(() => refreshStatus.current(true, true), CHECK_INTERVAL_MS);
    return () => { disposed = true; window.clearInterval(timer); };
  }, [repo, services]);

  const onEvent = (event: UpdateEvent): void => {
    switch (event.event) {
      case "checking": dispatch({ type: "check" }); return;
      case "downloadStarted": dispatch({ type: "download", version: event.data.version,
        downloaded: 0, contentLength: event.data.contentLength }); return;
      case "downloadProgress": dispatch({ type: "download", version: event.data.version,
        downloaded: event.data.downloaded, contentLength: event.data.contentLength }); return;
      case "verifying": dispatch({ type: "verify", version: event.data.version }); return;
      case "waitingForIdle": dispatch({ type: "wait", version: event.data.version }); return;
      case "installing": dispatch({ type: "install", version: event.data.version }); return;
      case "restarting": dispatch({ type: "restart", version: event.data.version }); return;
      default: {
        const exhaustive: never = event;
        return exhaustive;
      }
    }
  };

  const onUpdate = async (): Promise<void> => {
    if (updateInFlight.current || available?.repo !== repo) return;
    if (!repo.trim()) {
      dispatch({ type: "fail", message: t.updateNeedRepo });
      return;
    }
    updateInFlight.current = true;
    backendBusy.current = true;
    const operation = ++attempt.current;
    const generation = lifecycle.current;
    const isCurrent = (): boolean => generation === lifecycle.current && operation === attempt.current;
    let refreshAfter = false;
    setAttempted(true);
    dispatch({ type: "check" });
    try {
      const outcome = await services.updateApp(repo, (event) => { if (isCurrent()) onEvent(event); });
      if (!isCurrent()) return;
      if (outcome.status === "upToDate") {
        backendBusy.current = false;
        setAvailable(null);
        setAttempted(false);
        dispatch({ type: "uptodate" });
      }
      else dispatch({ type: "restart", version: outcome.version });
    } catch (error: unknown) {
      if (!isCurrent()) return;
      const raw = error instanceof Error ? error.message : error;
      if (raw === "in_progress") {
        refreshAfter = true;
      } else {
        backendBusy.current = false;
        dispatch({ type: "fail", message: updateErrorMessage(latestDict.current, error) });
      }
    } finally {
      if (isCurrent()) {
        updateInFlight.current = false;
        if (refreshAfter) refreshStatus.current();
        else scheduleStatus.current();
      }
    }
  };

  const onCancelWait = async (): Promise<void> => {
    if (state.kind !== "waitingForIdle" || state.canceling) return;
    dispatch({ type: "cancelWait" });
    const generation = lifecycle.current;
    try {
      await services.cancelUpdate();
      if (generation === lifecycle.current) refreshStatus.current();
    } catch (error: unknown) {
      if (generation !== lifecycle.current) return;
      dispatch({ type: "fail", message: updateErrorMessage(latestDict.current, error) });
    }
  };

  let status = "";
  let statusClass = "set-footer-status";
  switch (state.kind) {
    case "idle": break;
    case "checking": status = t.updateChecking; break;
    case "downloading": status = `${t.updateUpdating}${state.percent === null ? "" : ` ${state.percent}%`}`; break;
    case "verifying": status = t.updateVerifying; break;
    case "waitingForIdle": status = state.canceling ? t.updateCanceling : t.updateWaitingForIdle; break;
    case "installing": status = t.updateInstalling; break;
    case "restarting": status = t.updateRestarting; break;
    case "uptodate": break;
    case "error": status = state.message; statusClass += " set-footer-error"; break;
    default: {
      const exhaustive: never = state;
      status = exhaustive;
    }
  }

  const isBusy = busyStatuses.includes(state.kind);
  return (
    <footer className="set-footer">
      <span className="set-footer-version">{version ? `v${version}` : ""}</span>
      {(available?.repo === repo || isBusy) && (
        <button className="set-footer-btn set-footer-btn-accent" disabled={isBusy}
          onClick={() => void onUpdate()}>{t.updateCheck}</button>
      )}
      {state.kind === "waitingForIdle" && (
        <button className="set-footer-btn" disabled={state.canceling}
          onClick={() => void onCancelWait()}>{t.updateCancelWait}</button>
      )}
      {attempted && status && (
        <span className={statusClass} title={status} role="status">{status}</span>
      )}
    </footer>
  );
}
