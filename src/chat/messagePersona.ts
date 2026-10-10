import { findPersonaById, personaLabel } from "../pet/personaCatalog";

/** Stored in the native user message's system field, stable across prompt edits. */
export function personaSystemMarker(id: string): string {
  if (!/^[a-zA-Z0-9_-]{1,128}$/.test(id)) throw new Error("invalid_persona_id");
  return `<!-- yume:persona=${id} -->`;
}

export function historicalPersonaName(id: string | undefined, language: string): string {
  const persona = id ? findPersonaById(id) : undefined;
  if (persona) return personaLabel(persona, language);
  return ({ "zh-CN": "对方", "zh-TW": "對方", "ja-JP": "相手", "ko-KR": "상대" } as Record<string, string>)[language] ?? "Assistant";
}
