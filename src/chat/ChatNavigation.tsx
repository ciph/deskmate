import { useEffect, useId, useMemo, useRef, useState, type RefObject } from "react";
import { defaultRangeExtractor, useVirtualizer } from "@tanstack/react-virtual";
import { activeTurnIndex, type Turn } from "./conversationIndex";
import type { TimelineControl } from "./ChatTimeline";
import "./chat-navigation.css";
const COPY = {
  zh: { nav: "对话导航", attachment: "附件消息", waiting: "暂无回复", turn: (index: number) => `第 ${index} 轮` },
  en: { nav: "Conversation navigation", attachment: "Attachment message", waiting: "No reply yet", turn: (index: number) => `Turn ${index}` },
  ja: { nav: "会話ナビゲーション", attachment: "添付メッセージ", waiting: "返信はまだありません", turn: (index: number) => `会話 ${index}` },
  ko: { nav: "대화 탐색", attachment: "첨부 메시지", waiting: "아직 답변이 없습니다", turn: (index: number) => `대화 ${index}` },
} as const;

interface ChatNavigationProps {
  readonly turns: readonly Turn[];
  readonly listRef: RefObject<HTMLDivElement | null>;
  readonly controlRef: RefObject<TimelineControl | null>;
  readonly visibleIndex: number;
  readonly lang: string;
  readonly onNavigate: () => void;
}
export function ChatNavigation({ turns, listRef, controlRef, visibleIndex, lang, onNavigate }: ChatNavigationProps) {
  const active = activeTurnIndex(turns, visibleIndex);
  const [preview, setPreview] = useState<{ id: string; top: number } | null>(null);
  const [focused, setFocused] = useState<number | null>(null);
  const railRef = useRef<HTMLElement>(null);
  const tooltipId = useId();
  const copy = lang.startsWith("zh") ? COPY.zh : lang.startsWith("ja") ? COPY.ja : lang.startsWith("ko") ? COPY.ko : COPY.en;
  const selectedIndex = useMemo(() => turns.findIndex(turn => turn.id === preview?.id), [turns, preview?.id]);
  const selected = turns[selectedIndex];
  const virtual = useVirtualizer({ count: turns.length, getScrollElement: () => railRef.current,
    getItemKey: index => turns[index]!.id, estimateSize: () => 16, overscan: 8,
    initialRect: { width: 28, height: 320 },
    rangeExtractor: range => [...new Set([...defaultRangeExtractor(range), active, ...(focused === null ? [] : [focused]), ...(selectedIndex < 0 ? [] : [selectedIndex])])].filter(index => index < turns.length).sort((a,b) => a-b),
  });
  useEffect(() => { if (focused === null && !preview) virtual.scrollToIndex(active, { align: "auto" }); }, [active, focused, preview, virtual]);
  if (turns.length < 2) return null;
  function showPreview(id: string, button: HTMLElement) {
    const container = listRef.current?.parentElement;
    if (!container) return;
    const top = button.getBoundingClientRect().top - container.getBoundingClientRect().top;
    setPreview({ id, top: Math.max(8, Math.min(top - 24, container.clientHeight - 140)) });
  }
  return <div className="chat-navigation" onMouseLeave={() => setPreview(null)}
    onBlur={event => { if (!event.currentTarget.contains(event.relatedTarget)) { setPreview(null); setFocused(null); } }}
    onKeyDown={event => { if (event.key === "Escape") setPreview(null); }}>
    <nav className="chat-navigation-rail" aria-label={copy.nav} ref={railRef} style={{ height: Math.min(320, turns.length * 16) }}>
      <ul className="chat-navigation-items" style={{ height: virtual.getTotalSize(), position: "relative" }}>
      {virtual.getVirtualItems().map(item => { const turn = turns[item.index]!; return <li key={turn.id} aria-posinset={item.index + 1} aria-setsize={turns.length} style={{ position: "absolute", top: item.start, left: 0 }}><button type="button"
        data-turn-index={item.index}
        aria-label={`${copy.turn(item.index + 1)}：${turn.title || copy.attachment}`}
        aria-current={active === item.index ? "step" : undefined}
        aria-describedby={selected?.id === turn.id ? tooltipId : undefined}
        onMouseEnter={event => showPreview(turn.id, event.currentTarget)}
        onFocus={event => { setFocused(item.index); showPreview(turn.id, event.currentTarget); }}
        onKeyDown={event => {
          const next = event.key === "Home" ? 0 : event.key === "End" ? turns.length - 1 : event.key === "ArrowDown" ? item.index + 1 : event.key === "ArrowUp" ? item.index - 1 : -1;
          if (next < 0 || next >= turns.length) return;
          event.preventDefault(); setFocused(next); virtual.scrollToIndex(next, { align: "auto" });
          requestAnimationFrame(() => railRef.current?.querySelector<HTMLElement>(`[data-turn-index="${next}"]`)?.focus());
        }}
        onClick={() => { onNavigate(); controlRef.current?.jump(turn.index); setPreview(null); }}><span aria-hidden="true" /></button></li>; })}
      </ul>
    </nav>
    {selected && preview && <div id={tooltipId} role="tooltip" className="chat-navigation-preview" style={{ top: preview.top }}>
      <strong>{selected.title || copy.attachment}</strong><p>{selected.summary || copy.waiting}</p>
    </div>}
  </div>;
}
