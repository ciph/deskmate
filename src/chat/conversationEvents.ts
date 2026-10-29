import { subscribeEvents, type OpenCodeEvent } from "../lib/opencode";

export interface ConversationEventScope {
  readonly key: string;
  readonly directory: string;
  readonly sessionId: string;
  readonly generation: number;
}
export function sameEventScope(a: ConversationEventScope | null, b: ConversationEventScope) {
  return !!a && a.key === b.key && a.directory === b.directory && a.sessionId === b.sessionId && a.generation === b.generation;
}

/** A snapshot has no SSE cursor. Never replay ambiguous text deltas over it. */
export function createConversationEvents(
  deliver: (event: OpenCodeEvent) => void,
  reconcile: (scope: ConversationEventScope) => Promise<void>,
  subscribe = subscribeEvents,
) {
  type Connection = { scope: ConversationEventScope; events: OpenCodeEvent[]; buffering: boolean;
    stop?: () => void; ready: Promise<void>; resolve: () => void; reject: (error: Error) => void;
    timer?: ReturnType<typeof setTimeout>; recovering: boolean; dirty: boolean; needsFinal: boolean };
  let current: Connection | null = null;

  function close() {
    const previous = current;
    current = null;
    if (!previous) return;
    clearTimeout(previous.timer);
    previous.stop?.();
    previous.reject(new Error("Conversation subscription replaced"));
    previous.events.length = 0;
  }
  function flush(connection: Connection) {
    if (connection !== current) return;
    if (connection.needsFinal && connection.events.some(event => event.type === "session.idle" || event.type === "session.error")) {
      connection.needsFinal = false;
      void recover(connection);
      return;
    }
    connection.buffering = false;
    const events = connection.events.splice(0);
    for (const event of events) {
      if (connection !== current) break;
      deliver(event);
    }
  }
  async function recover(connection: Connection) {
    connection.buffering = true;
    if (connection.recovering) { connection.dirty = true; return; }
    connection.recovering = true;
    try {
      do {
        connection.dirty = false;
        await reconcile(connection.scope);
      } while (connection === current && connection.dirty);
      // The owner flushes after the snapshot has committed into React state.
    } catch (error) {
      if (connection === current) {
        // An unavailable host must not create an immediate retry/read loop.
        // Retain the gap for the next reconnect or terminal event.
        connection.buffering = false;
        connection.needsFinal = true;
        for (const event of connection.events.splice(0)) deliver(event);
        console.warn("Conversation recovery unavailable", error instanceof Error ? error.message : "unavailable");
      }
    } finally { connection.recovering = false; }
  }
  return {
    close,
    matches(scope: ConversationEventScope) { return sameEventScope(current?.scope ?? null, scope); },
    async prepare(scope: ConversationEventScope) {
      if (current && sameEventScope(current.scope, scope)) return current.ready;
      close();
      let resolve = () => {};
      let reject = (_error: Error) => {};
      const ready = new Promise<void>((yes, no) => { resolve = yes; reject = no; });
      // The connection can be disposed before its caller starts awaiting readiness.
      void ready.catch(() => {});
      const connection: Connection = { scope, ready, resolve, reject, events: [], buffering: true, recovering: false, dirty: false, needsFinal: false };
      current = connection;
      connection.timer = setTimeout(() => {
        if (current === connection) { reject(new Error("Conversation event connection timed out")); close(); }
      }, 120_000);
      void subscribe(event => {
        if (connection !== current) return;
        const props = event.properties ?? {};
        const source = (props.part ?? props.info ?? props) as { sessionID?: string };
        if (source.sessionID && source.sessionID !== scope.sessionId) return;
        const terminal = event.type === "session.idle" || event.type === "session.error";
        if (connection.buffering && event.type.startsWith("message.")) {
          // These may precede OR follow the snapshot's read. One authoritative
          // terminal read settles the gap without duplicating/reverting text.
          connection.needsFinal = true;
        } else if (connection.buffering || (terminal && connection.needsFinal)) {
          // Status events are replaceable; never buffer a global event stream.
          if (terminal || event.type === "session.status") {
            connection.events = connection.events.filter(previous => previous.type !== event.type);
            connection.events.push(event);
          }
          if (!connection.buffering) { connection.needsFinal = false; void recover(connection); }
        } else deliver(event);
      }, scope.directory, reconnected => {
        if (connection !== current) return;
        clearTimeout(connection.timer);
        resolve();
        if (reconnected) void recover(connection);
      }).then(stop => {
        if (connection !== current) stop(); else connection.stop = stop;
      }, error => {
        reject(error instanceof Error ? error : new Error("Event subscription failed"));
        if (current === connection) close();
      });
      return ready;
    },
    flush(scope: ConversationEventScope) {
      if (current && sameEventScope(current.scope, scope)) flush(current);
    },
    markGap() { if (current) current.needsFinal = true; },
    snapshot(scope: ConversationEventScope, active: boolean) {
      if (active && current && !current.recovering && sameEventScope(current.scope, scope)) current.needsFinal = true;
    },
  };
}
