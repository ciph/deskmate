import { useEffect, useState } from "react";
import { cancelWindowsComponent, installWindowsComponent, retryWindowsComponentActivation, setWindowsComponentEnabled, windowsComponentStatus, type WindowsComponentStatus } from "../lib/windowsComponent";
import { windowsComponentText } from "./windowsComponentText";

function useComponentSession() {
  const [status, setStatus] = useState<WindowsComponentStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const [failed, setFailed] = useState(false);
  useEffect(() => {
    let stopped = false;
    let timer: ReturnType<typeof setTimeout> | undefined;
    async function poll() {
      try {
        const next = await windowsComponentStatus();
        if (!stopped) setStatus(next);
      } catch { if (!stopped) setFailed(true); }
      if (!stopped) timer = setTimeout(() => void poll(), 1000);
    }
    void poll();
    return () => { stopped = true; clearTimeout(timer); };
  }, []);
  async function act(action: () => Promise<void>) {
    if (busy) return;
    setBusy(true); setFailed(false);
    try { await action(); setStatus(await windowsComponentStatus()); return true; }
    catch { setFailed(true); }
    finally { setBusy(false); }
  }
  return { status, busy, failed, setFailed, act };
}

export function WindowsComponentControl({ enabled, language, label }: { readonly enabled: boolean; readonly language: string; readonly label: string }) {
  const t = windowsComponentText(language);
  const { status, busy, failed, setFailed, act } = useComponentSession();
  const [pending, setPending] = useState<boolean | null>(null);
  const downloading = status?.phase === "downloading" || status?.phase === "verifying" || status?.phase === "cancelling";
  const ready = status?.phase === "ready";
  const needsDownload = status?.supported && !ready && !downloading;
  const errorText = status?.error === "component_checksum" ? t.checksum : status?.error === "component_storage" ? t.storage : t.failed;
  const mib = ((status?.total ?? 0) / 1048576).toFixed(1);
  const confirm = () => pending === false ? setWindowsComponentEnabled(false)
    : ready ? setWindowsComponentEnabled(true) : installWindowsComponent();
  async function applyChange() {
    if (await act(confirm)) setPending(null);
  }

  return <div className="set-permissions-component">
    <label className="set-permissions-row set-permissions-switch" htmlFor="set-permissions-windows">
      <span>{label}</span>
      <input id="set-permissions-windows" type="checkbox" checked={enabled}
        disabled={!status?.supported || busy || downloading || status.activation === "applying"}
        onChange={(event) => { setFailed(false); setPending(event.target.checked); }} />
    </label>
    {!status && <p className="set-note" role="status">{t.checking}</p>}
    {status && !status.supported && <p className="set-note">{t.unsupported}</p>}
    {needsDownload && <div className="set-note">
      <p>{t.missing}</p><p>{t.size(mib)}</p>
      {pending === null && <button type="button" className="set-permissions-link" disabled={busy} onClick={() => setPending(true)}>{t.download}</button>}
    </div>}
    {ready && <p className="set-note">{t.ready} · {status.version}</p>}
    {downloading && <div role="status" className="set-note">
      <p>{status.phase === "cancelling" ? t.cancelling : status.phase === "verifying" ? t.verifying : t.downloading}</p>
      <progress aria-label={t.downloading} max={status.total} value={status.downloaded} />
      <span> {((status.downloaded / Math.max(status.total, 1)) * 100).toFixed(0)}%</span>
      <button type="button" className="set-permissions-link" disabled={busy || status.phase === "cancelling"} onClick={() => void act(cancelWindowsComponent)}>{t.cancelDownload}</button>
    </div>}
    {status?.activation === "waiting" && <p className="set-note" role="status">{t.waiting}</p>}
    {status?.activation === "applying" && <p className="set-note" role="status">{t.applying}</p>}
    {(failed || status?.phase === "error" || status?.activation === "error") && <div className="set-permissions-feedback" role="alert">
      <p>{errorText}</p>
      {ready && <button type="button" className="set-permissions-link" disabled={busy} onClick={() => void act(status.error === "component_enable" ? () => setWindowsComponentEnabled(true) : retryWindowsComponentActivation)}>{t.retry}</button>}
      {!status && <button type="button" className="set-permissions-link" disabled={busy} onClick={() => void act(async () => {})}>{t.retry}</button>}
    </div>}
    {pending !== null && !downloading && <div className="set-permissions-confirm" role="group" aria-label={pending ? t.enable : t.disable}>
      <strong>{pending ? t.enable : t.disable}</strong>
      <p>{t.safeApply}</p>
      {pending && !ready && <p>{t.size(mib)}</p>}
      <div className="set-permissions-confirm-actions">
        <button type="button" className="set-permissions-apply" disabled={busy} onClick={() => void applyChange()}>{pending && !ready ? t.download : t.apply}</button>
        <button type="button" className="set-permissions-link" disabled={busy} onClick={() => setPending(null)}>{t.cancel}</button>
      </div>
    </div>}
  </div>;
}
