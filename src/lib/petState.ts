import { emit, listen, type UnlistenFn } from "@tauri-apps/api/event";

/** Chat and agent activity share one ordered channel for every pet renderer. */
export type PetMood = "idle" | "thinking" | "talking" | "working" | "error";

export type PetActivityScope = {
  readonly sessionId: string;
  readonly requestId: string;
};
export type PetActivityInput = PetActivityScope & (
  | { readonly type: "start" }
  | { readonly type: "success" }
  | { readonly type: "error" }
  | { readonly type: "cancel" }
  | { readonly type: "mood"; readonly mood: PetMood }
);
export type PetActivityEvent = PetActivityInput & {
  readonly producerEpoch: number;
  readonly sequence: number;
  readonly eventId: string;
};
const ACTIVITY_EVENT = "deskmate://pet-activity";
const EPOCH_KEY = "yume.petActivity.epoch";
let latestEpoch = 0;

export function createPetActivitySequencer(
  storage: () => Pick<Storage, "getItem" | "setItem"> = () => localStorage,
  now: () => number = Date.now,
): (event: PetActivityInput) => PetActivityEvent {
  let producerEpoch = 0;
  let sequence = 0;
  return (event) => {
    if (producerEpoch === 0 || sequence === Number.MAX_SAFE_INTEGER) {
      let previous = 0;
      try {
        const stored = Number(storage().getItem(EPOCH_KEY));
        if (Number.isSafeInteger(stored) && stored > 0 && stored < Number.MAX_SAFE_INTEGER) previous = stored;
      } catch { /* Storage may be disabled; the current page still has a monotonic epoch. */ }
      producerEpoch = Math.max(previous, latestEpoch, Math.floor(now())) + 1;
      latestEpoch = producerEpoch;
      sequence = 0;
      try { storage().setItem(EPOCH_KEY, String(producerEpoch)); }
      catch { /* shortcut: reload plus a backward clock needs persistent storage to retain ordering. */ }
    }
    sequence += 1;
    return { ...event, producerEpoch, sequence, eventId: String(producerEpoch) + ":" + sequence };
  };
}

const sequenceActivity = createPetActivitySequencer();

export function broadcastPetActivity(event: PetActivityInput): void {
  void emit(ACTIVITY_EVENT, sequenceActivity(event));
}

export function isPetActivityEvent(value: unknown): value is PetActivityEvent {
  if (typeof value !== "object" || value === null) return false;
  const event = value as Record<string, unknown>;
  if (typeof event.sessionId !== "string" || !event.sessionId || typeof event.requestId !== "string" || !event.requestId
    || !Number.isSafeInteger(event.producerEpoch) || Number(event.producerEpoch) <= 0
    || !Number.isSafeInteger(event.sequence) || Number(event.sequence) <= 0
    || event.eventId !== String(event.producerEpoch) + ":" + event.sequence) return false;
  if (event.type === "mood") return typeof event.mood === "string" && ["idle", "thinking", "talking", "working", "error"].includes(event.mood);
  return event.type === "start" || event.type === "success" || event.type === "error" || event.type === "cancel";
}

export function onPetActivity(cb: (event: PetActivityEvent) => void): Promise<UnlistenFn> {
  return listen<unknown>(ACTIVITY_EVENT, (event) => {
    if (isPetActivityEvent(event.payload)) cb(event.payload);
  });
}
