import type { PetActivityEvent, PetMood } from "../lib/petState";


export interface GifState {
  readonly request: { readonly sessionId: string; readonly requestId: string } | null;
  readonly terminal: false | "success" | "error" | "cancel";
  readonly base: PetMood;
  readonly thinkingSince: number | null;
  readonly thinkingVariant: "thinking" | "working";
  readonly feedback: { readonly state: "success" | "error"; readonly until: number } | null;
  readonly producerEpoch: number;
  readonly sequence: number;
}
export const initialGifState: GifState = {
  thinkingVariant: "thinking", request: null, terminal: false, base: "idle", thinkingSince: null, feedback: null,
  producerEpoch: 0, sequence: 0,
};
export interface GifTiming {
  readonly successMs: number;
  readonly errorMs: number;
  readonly thinkingEscalationMs: number;
}
export type GifPlayback = GifTiming | { readonly successMs: number; readonly errorMs: number; readonly thinkingSelection: "random" };
export const defaultGifTiming: GifTiming = { successMs: 1400, errorMs: 1440, thinkingEscalationMs: 8000 };

export function reduceGifState(
  state: GifState, event: PetActivityEvent, now: number, visible = true,
  timing: GifPlayback = defaultGifTiming,
): GifState {
  const { producerEpoch, sequence } = event;
  if (!Number.isSafeInteger(producerEpoch) || producerEpoch < 1 || !Number.isSafeInteger(sequence) || sequence < 1
    || event.eventId !== `${producerEpoch}:${sequence}`) return state;
  // A producer-wide watermark survives terminal events and persona changes without retaining request history.
  if (producerEpoch < state.producerEpoch || (producerEpoch === state.producerEpoch && sequence <= state.sequence)) return state;
  const sameRequest = state.request?.sessionId === event.sessionId && state.request.requestId === event.requestId;
  if (event.type === "start") {
    if (sameRequest) return state;
    return { thinkingVariant: "thinking", request: { sessionId: event.sessionId, requestId: event.requestId },
      terminal: false, base: "thinking", thinkingSince: now, feedback: null, producerEpoch, sequence };
  }
  if (producerEpoch !== state.producerEpoch || !sameRequest || state.terminal) return state;
  const accepted = { ...state, sequence };
  switch (event.type) {
    case "mood":
      return { ...accepted, base: event.mood,
        thinkingSince: event.mood === "thinking" ? state.thinkingSince ?? now : null };
    case "success":
    case "error":
      return { ...accepted, terminal: event.type, base: "idle", thinkingSince: null,
        feedback: visible ? { state: event.type, until: now + (event.type === "success" ? timing.successMs : timing.errorMs) } : null };
    case "cancel":
      return { ...accepted, terminal: event.type, base: "idle", thinkingSince: null, feedback: null };
    default: {
      const exhaustive: never = event;
      return exhaustive;
    }
  }
}

export function gifDisplayState(state: GifState, now: number, timing: GifPlayback = defaultGifTiming): PetMood | "success" {
  if (state.feedback !== null && now < state.feedback.until) return state.feedback.state;
  if (state.base === "thinking" && "thinkingSelection" in timing) return state.thinkingVariant;
  if (state.base === "thinking" && state.thinkingSince !== null && "thinkingEscalationMs" in timing && now - state.thinkingSince >= timing.thinkingEscalationMs) return "working";
  return state.base;
}

export function gifNextDeadline(state: GifState, now: number, timing: GifPlayback): number | null {
  if (state.feedback !== null && state.feedback.until > now) return state.feedback.until;
  if ("thinkingEscalationMs" in timing && state.thinkingSince !== null && state.thinkingSince + timing.thinkingEscalationMs > now) return state.thinkingSince + timing.thinkingEscalationMs;
  return null;
}

/** Called at the event boundary; the pure reducer never consumes randomness. */
export function applyGifEvent(
  state: GifState, event: PetActivityEvent, now: number, visible: boolean,
  playback: GifPlayback, random: () => number,
): GifState {
  const next = reduceGifState(state, event, now, visible, playback);
  if (next === state || !("thinkingSelection" in playback)) return next;
  const entered = next.base === "thinking" && (event.type === "start" || state.base !== "thinking");
  return entered ? { ...next, thinkingVariant: random() < 0.5 ? "thinking" : "working" } : next;
}

export function gifIdleEligible(state: GifState, now: number): boolean {
  return (state.request === null || Boolean(state.terminal)) && state.base === "idle"
    && (state.feedback === null || now >= state.feedback.until);
}
