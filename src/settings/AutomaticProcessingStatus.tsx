import { useCallback, useContext, useState } from "react";
import type { Dict } from "../lib/i18n";
import { memoryAutomationStatus, retryMemoryAutomation, type AutomationStatus } from "../lib/memory";
import { SettingsVisibilityContext } from "./settingsVisibility";
import { useVisiblePoll } from "./useVisiblePoll";

/** Visible only in management screens, never below messages. */
export function AutomaticProcessingStatus({ t }: { t: Dict }) {
  const [status, setStatus] = useState<AutomationStatus | null>(null);
  const [retrying, setRetrying] = useState(false);
  const visible = useContext(SettingsVisibilityContext);
  const read = useCallback(async (current: () => boolean) => {
    try { const next = await memoryAutomationStatus(); if (current()) setStatus(next ?? null); }
    catch { if (current()) setStatus(null); }
  }, []);
  const refresh = useVisiblePoll(read, visible, 5000);
  async function retry() {
    setRetrying(true);
    try { await retryMemoryAutomation(); refresh(); }
    catch { /* Keep the failure visible; no false success message. */ }
    finally { setRetrying(false); }
  }
  if (!status || (!status.pending && !status.failed)) return null;
  return <div className="set-note" role="status">
    {status.pending > 0 && <p>{t.automaticProcessingPending}</p>}
    {status.failed > 0 && <p>{t.automaticProcessingFailed} <button className="set-btn" type="button" disabled={retrying} onClick={() => { void retry(); }}>{t.automaticProcessingRetry}</button></p>}
  </div>;
}
