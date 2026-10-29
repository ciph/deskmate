import { asWorklogError, getOperationStates, type OperationReceipt, type OperationState, type RunSummary, type ScheduleSummary } from "../lib/worklog";
import type { WorklogOutcome } from "../lib/unifiedHistory";

export interface ChatWorklogOperation {
  readonly messageId: string; readonly requestId: string;
  readonly receipt: OperationReceipt | null; readonly error: string | null;
  readonly run?: RunSummary; readonly schedule?: ScheduleSummary; readonly undoable?: boolean;
}

/** One view owns one queue. Closed views cannot publish or start another batch. */
export function createWorklogRecovery(
  publish: (operations: readonly ChatWorklogOperation[]) => void,
  lookup = getOperationStates,
) {
  const operations = new Map<string, ChatWorklogOperation>();
  const versions = new Map<string, number>();
  const pending = new Set<string>();
  let closed = false;
  let flight: Promise<void> | null = null;
  const emit = () => { if (!closed) publish([...operations.values()]); };

  function apply(state: OperationState, expected: number) {
    const current = operations.get(state.requestId);
    if (!current || versions.get(state.requestId) !== expected || !state.receipt || state.receipt.operationId !== state.requestId) return;
    // A missing lookup never erases a known rejection; only the host commits facts.
    operations.set(state.requestId, { ...current, receipt: state.receipt,
      run: state.run ?? undefined, schedule: state.schedule ?? undefined,
      error: state.entityMissing ? "NOT_FOUND" : null });
  }

  function request(ids: Iterable<string>): Promise<void> {
    if (closed) return Promise.resolve();
    for (const id of ids) {
      if (!operations.has(id)) continue;
      pending.add(id);
      versions.set(id, (versions.get(id) ?? 0) + 1);
    }
    if (flight) return flight;
    if (!pending.size) return Promise.resolve();
    flight = Promise.resolve().then(async () => {
      while (!closed && pending.size) {
        const batch = [...pending].slice(0, 50);
        const expected = new Map(batch.map(id => [id, versions.get(id)!]));
        for (const id of batch) pending.delete(id);
        try {
          const states = await lookup(batch);
          if (closed) break;
          for (const state of states) if (expected.has(state.requestId)) apply(state, expected.get(state.requestId)!);
        } catch (error) {
          if (closed) break;
          for (const id of batch) {
            const current = operations.get(id);
            if (current && versions.get(id) === expected.get(id) && !current.error) {
              operations.set(id, { ...current, error: asWorklogError(error).code });
            }
          }
        }
        emit();
      }
    }).finally(() => { flight = null; });
    return flight;
  }

  return {
    close() { closed = true; pending.clear(); operations.clear(); versions.clear(); },
    get closed() { return closed; },
    accept(outcomes: readonly WorklogOutcome[]) {
      if (closed || !outcomes.length) return;
      for (const outcome of outcomes) {
        const previous = operations.get(outcome.requestId);
        operations.set(outcome.requestId, previous ? {
          ...previous, error: previous.receipt ? previous.error : outcome.error ?? previous.error,
        } : { ...outcome, receipt: null });
      }
      emit();
      void request(outcomes.map(outcome => outcome.requestId));
    },
    update(operation: ChatWorklogOperation) {
      if (closed) return;
      operations.set(operation.requestId, operation);
      emit();
    },
    get(id: string) { return operations.get(id); },
    refresh(id?: string) { return request(id ? [id] : operations.keys()); },
  };
}
