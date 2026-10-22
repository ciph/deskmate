/** Full text snapshots, not deltas: repeat events replace their own part only. */
export interface AssistantTextSnapshot {
  readonly messageId: string;
  readonly text: string;
}

export const STREAM_TEXT_INTERVAL_MS = 33;

/** Bound presentation work while preserving the first content and terminal flush. */
export function createStreamTextBuffer(publish: (updates: readonly AssistantTextSnapshot[]) => void) {
  const parts = new Map<string, Map<string, string>>();
  const pending = new Map<string, string>();
  const presented = new Set<string>();
  const invalidated = new Set<string>();
  let timer: ReturnType<typeof setTimeout> | undefined;
  let generation = 0;

  function flush() {
    if (timer !== undefined) clearTimeout(timer);
    timer = undefined;
    if (!pending.size) return;
    const updates = Array.from(pending, ([messageId, text]) => ({ messageId, text }));
    pending.clear();
    publish(updates);
  }

  return {
    push(messageId: string, partId: string | undefined, text: string) {
      let messageParts = parts.get(messageId);
      if (!messageParts) {
        messageParts = new Map();
        parts.set(messageId, messageParts);
      }
      const key = partId ?? messageId;
      if (messageParts.get(key) === text && !invalidated.has(messageId)) return;
      invalidated.delete(messageId);
      messageParts.set(key, text);
      const combined = Array.from(messageParts.values()).join("");
      pending.set(messageId, combined);
      if (!presented.has(messageId) && combined.length) {
        presented.add(messageId);
        flush();
      } else if (timer === undefined) {
        const expected = generation;
        timer = setTimeout(() => { if (expected === generation) flush(); }, STREAM_TEXT_INTERVAL_MS);
      }
    },
    flush,
    invalidate(messageId: string) { invalidated.add(messageId); },
    restore(snapshots: readonly { messageId?: string; partId?: string; text: string }[]) {
      for (const snapshot of snapshots) {
        if (!snapshot.messageId) continue;
        let messageParts = parts.get(snapshot.messageId);
        if (!messageParts) {
          messageParts = new Map();
          parts.set(snapshot.messageId, messageParts);
        }
        messageParts.set(snapshot.partId ?? snapshot.messageId, snapshot.text);
        if (snapshot.text.length) presented.add(snapshot.messageId);
      }
    },
    reset() {
      generation += 1;
      if (timer !== undefined) clearTimeout(timer);
      timer = undefined;
      pending.clear();
      parts.clear();
      presented.clear();
      invalidated.clear();
    },
  };
}
