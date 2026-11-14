import type { CatalogLoadResult } from "../lib/unifiedHistory";
import type { HistoryMessage } from "../lib/history";

function key(message: HistoryMessage) {
  return message.messageId ? JSON.stringify([message.messageId, message.partId ?? ""]) : JSON.stringify([message.role, message.time, message.text]);
}
function order(a: HistoryMessage, b: HistoryMessage) {
  const compare = (left: string, right: string) => left === right ? 0 : left < right ? -1 : 1;
  return a.time - b.time || compare(a.messageId ?? "", b.messageId ?? "") || compare(a.partId ?? "", b.partId ?? "");
}
export function mergeHistoryPage(current: CatalogLoadResult, page: CatalogLoadResult, older: boolean): CatalogLoadResult {
  if (current.entry.key !== page.entry.key || JSON.stringify(current.entry.identity) !== JSON.stringify(page.entry.identity)) throw new Error("history identity changed");
  const native = page.messages.filter(message => !message.localOnly && message.messageId);
  const first = native.reduce<HistoryMessage | undefined>((found, message) => !found || order(message, found) < 0 ? message : found, undefined);
  const ids = new Set(native.map(message => message.messageId));
  // A compaction or a gap larger than one page requires a fresh cursor chain.
  if (!older && (!first || !current.messages.some(message => ids.has(message.messageId)))) return page;
  const retained = older ? current.messages : current.messages.filter(message => !message.localOnly && first &&
    (message.time < first.time || (message.time === first.time && (message.messageId ?? "") < (first.messageId ?? ""))));
  const messages = new Map(retained.map(message => [key(message), message]));
  // A previously loaded newer row wins when pages overlap during a live update.
  for (const message of page.messages) if (!older || !messages.has(key(message))) messages.set(key(message), message);
  const outcomes = new Map((older ? page.worklogOutcomes ?? [] : current.worklogOutcomes ?? []).map(item => [item.requestId, item]));
  for (const item of older ? current.worklogOutcomes ?? [] : page.worklogOutcomes ?? []) outcomes.set(item.requestId, item);
  return { ...page, nextCursor: older ? page.nextCursor : current.nextCursor,
    messages: [...messages.values()].sort(order), worklogOutcomes: [...outcomes.values()] };
}
