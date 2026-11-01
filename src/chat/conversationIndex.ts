import { stripLocalResourceContext } from "./localResources";
export interface NavigationMessage { readonly id: string; readonly role: string; readonly text?: string }
export interface Turn { readonly id: string; readonly title: string; readonly summary: string; readonly index: number }
export function conversationTurns(messages: readonly NavigationMessage[]) {
  return createConversationIndex()(messages).turns.map(({ id, title, summary }) => ({ id, title, summary }));
}
export function activeTurnIndex(turns: readonly Turn[], visible: number) {
  let low = 0, high = turns.length - 1;
  while (low < high) { const mid = Math.ceil((low + high) / 2); if (turns[mid]!.index <= visible) low = mid; else high = mid - 1; }
  return low;
}
function preview(text: string) {
  return text.replace(/!\[([^\]]*)\]\([^)]*\)/gu, "$1").replace(/\[([^\]]+)\]\([^)]*\)/gu, "$1")
    .replace(/[`*_>#~]/gu, "").replace(/\s+/gu, " ").trim();
}
/** Text changes reuse membership/position indexes and only rebuild their turn. */
export function createConversationIndex() {
  let previous: readonly NavigationMessage[] = [];
  let ids = new Set<string>(), positions = new Map<string, number>();
  let owners: number[] = [], starts: number[] = [], turns: Turn[] = [];
  let lastUser: string | undefined;
  const previews = new WeakMap<NavigationMessage, string>();
  function text(message: NavigationMessage) {
    let value = previews.get(message);
    if (value === undefined) {
      value = preview(message.role === "user" ? stripLocalResourceContext(message.text ?? "") : message.text ?? "");
      previews.set(message, value);
    }
    return value;
  }
  return (messages: readonly NavigationMessage[]) => {
    if (messages !== previous) {
      const changed: number[] = [];
      let structural = previous.length !== messages.length;
      for (let i = 0; i < messages.length; i += 1) {
        if (messages[i] === previous[i]) continue;
        changed.push(i);
        if (messages[i]!.id !== previous[i]?.id || messages[i]!.role !== previous[i]?.role) structural = true;
      }
      if (structural) {
        ids = new Set(); positions = new Map(); owners = []; starts = []; lastUser = undefined;
        messages.forEach((message, i) => {
          ids.add(message.id); positions.set(message.id, i);
          if (message.role === "user") { starts.push(i); lastUser = message.id; }
          owners.push(starts.length - 1);
        });
      }
      const dirty = structural ? starts.map((_, i) => i) : [...new Set(changed.map(i => owners[i]!).filter(i => i >= 0))];
      const next = structural ? [] : turns.slice();
      for (const turn of dirty) {
        const index = starts[turn]!, message = messages[index]!;
        let summary = "";
        for (let i = index + 1; i < (starts[turn + 1] ?? messages.length) && summary.length < 240; i += 1) {
          if (messages[i]!.role === "assistant") summary = `${summary} ${text(messages[i]!)}`.trim().slice(0, 240);
        }
        const title = text(message).slice(0, 160), old = turns[turn];
        next[turn] = old?.id === message.id && old.title === title && old.summary === summary && old.index === index
          ? old : { id: message.id, title, summary, index };
      }
      if (structural || dirty.some(i => next[i] !== turns[i])) turns = next;
      previous = messages;
    }
    return { ids, positions, turns, lastUser };
  };
}
