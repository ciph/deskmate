import { useCallback, useEffect, useLayoutEffect, useMemo, useState } from "react";
import { asWorklogError, deleteEntry, onWorklogChanged, recordEntry, saveSchedule } from "../lib/worklog";
import type { WorklogOutcome } from "../lib/unifiedHistory";
import { localWorklogDate, worklogOutput, WORKLOG_TOOLS } from "./worklogActions";
import { createWorklogRecovery, type ChatWorklogOperation } from "./worklogRecovery";
export type { ChatWorklogOperation } from "./worklogRecovery";

export function useWorklogChat(sessionId: string | null, directory?: string, viewKey = "", snapshot?: readonly WorklogOutcome[]) {
  const [operations, setOperations] = useState<readonly ChatWorklogOperation[]>([]);
  const scope = useMemo(() => ({ sessionId, directory, viewKey }), [sessionId, directory, viewKey]);
  const recovery = useMemo(() => ({ scope, store: createWorklogRecovery(setOperations) }), [scope]);
  useLayoutEffect(() => {
    // StrictMode may replay this effect after cleanup without a new render.
    if (recovery.store.closed) recovery.store = createWorklogRecovery(setOperations);
    const store = recovery.store;
    setOperations([]);
    return () => store.close();
  }, [recovery]);
  useEffect(() => { if (snapshot) recovery.store.accept(snapshot); }, [recovery, snapshot]);
  useEffect(() => {
    const stop = onWorklogChanged(() => { void recovery.store.refresh(); }).catch((error: unknown) => {
      console.warn("Work journal live refresh unavailable", asWorklogError(error).code);
      return null;
    });
    return () => { void stop.then(unlisten => unlisten?.()); };
  }, [recovery]);

  const acceptTool = useCallback((part: unknown) => {
    if (typeof part !== "object" || part === null || !("tool" in part) || !("messageID" in part) || !("state" in part)) return;
    if (typeof part.tool !== "string" || !WORKLOG_TOOLS.some(tool => tool === part.tool) || typeof part.messageID !== "string" || part.tool === "worklog_query") return;
    if (typeof part.state !== "object" || part.state === null || !("output" in part.state)) return;
    const output = worklogOutput(part.state.output);
    if (output) recovery.store.accept([{ ...output, messageId: part.messageID, undoable: part.tool === "worklog_record" }]);
  }, [recovery]);

  const refresh = useCallback(async (operation: ChatWorklogOperation) => { await recovery.store.refresh(operation.requestId); }, [recovery]);
  const refreshKnown = useCallback(async () => { await recovery.store.refresh(); }, [recovery]);
  const save = useCallback(async (messageId: string, text: string) => {
    const requestId = crypto.randomUUID();
    const operation: ChatWorklogOperation = { messageId, requestId, receipt: null, error: null, undoable: true };
    recovery.store.update(operation);
    try {
      await recordEntry({ requestId, businessDate: localWorklogDate(), project: null, originalText: text, text, status: "done", sourceSessionId: scope.sessionId, sourceMessageId: messageId });
      await recovery.store.refresh(requestId);
    } catch (error: unknown) { recovery.store.update({ ...operation, error: asWorklogError(error).code }); }
  }, [recovery, scope]);
  const schedule = useCallback(async (messageId: string) => {
    const requestId = crypto.randomUUID();
    const operation: ChatWorklogOperation = { messageId, requestId, receipt: null, error: null };
    recovery.store.update(operation);
    try {
      await saveSchedule({ requestId, id: null, expectedRevision: null, kind: "weekly", weekdaySet: [5], localTime: "17:00", enabled: true });
      await recovery.store.refresh(requestId);
    } catch (error: unknown) { recovery.store.update({ ...operation, error: asWorklogError(error).code }); }
  }, [recovery]);
  const undo = useCallback(async (operation: ChatWorklogOperation) => {
    const receipt = operation.receipt;
    if (!receipt || receipt.entityKind !== "entry") return;
    try {
      await deleteEntry({ requestId: crypto.randomUUID(), id: receipt.entityId, expectedRevision: receipt.revision, deleteLinkedReports: false });
      recovery.store.update({ ...operation, undoable: false });
      await recovery.store.refresh(operation.requestId);
    } catch (error: unknown) { recovery.store.update({ ...operation, error: asWorklogError(error).code }); }
  }, [recovery]);
  return { operations, acceptTool, refreshKnown, save, schedule, undo, refresh };
}
