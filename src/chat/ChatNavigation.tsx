import { useEffect, useId, useMemo, useRef, useState, type RefObject } from "react";
import "./chat-navigation.css";
import { stripLocalResourceContext } from "./localResources";

interface NavigationMessage {
  readonly id: string;
  readonly role: string;
  readonly text?: string;
}
interface Turn {
  readonly id: string;
  readonly title: string;
  readonly summary: string;
}

function previewText(text: string): string {
  return text.replace(/!\[([^\]]*)\]\([^)]*\)/gu, "$1")
    .replace(/\[([^\]]+)\]\([^)]*\)/gu, "$1")
    .replace(/[`*_>#~]/gu, "").replace(/\s+/gu, " ").trim();
}

export function conversationTurns(messages: readonly NavigationMessage[]): readonly Turn[] {
  const turns: Turn[] = [];
  for (const message of messages) {
    if (message.role === "user") {
      turns.push({ id: message.id, title: previewText(stripLocalResourceContext(message.text ?? "")).slice(0, 160), summary: "" });
    } else if (message.role === "assistant" && message.text?.trim()) {
      const previous = turns.at(-1);
      if (previous && previous.summary.length < 240) {
        turns[turns.length - 1] = { ...previous, summary: previewText(`${previous.summary} ${message.text}`).slice(0, 240) };
      }
    }
  }
  return turns;
}

const COPY = {
  zh: { nav: "对话导航", attachment: "附件消息", waiting: "暂无回复", turn: (index: number) => `第 ${index} 轮` },
  en: { nav: "Conversation navigation", attachment: "Attachment message", waiting: "No reply yet", turn: (index: number) => `Turn ${index}` },
  ja: { nav: "会話ナビゲーション", attachment: "添付メッセージ", waiting: "返信はまだありません", turn: (index: number) => `会話 ${index}` },
  ko: { nav: "대화 탐색", attachment: "첨부 메시지", waiting: "아직 답변이 없습니다", turn: (index: number) => `대화 ${index}` },
} as const;

interface ChatNavigationProps {
  readonly messages: readonly NavigationMessage[];
  readonly listRef: RefObject<HTMLDivElement | null>;
  readonly lang: string;
  readonly onNavigate: () => void;
}

export function ChatNavigation({ messages, listRef, lang, onNavigate }: ChatNavigationProps) {
  const turns = useMemo(() => conversationTurns(messages), [messages]);
  const [active, setActive] = useState("");
  const [preview, setPreview] = useState<{ readonly id: string; readonly top: number } | null>(null);
  const railRef = useRef<HTMLElement>(null);
  const tooltipId = useId();
  const copy = lang.startsWith("zh") ? COPY.zh : lang.startsWith("ja") ? COPY.ja : lang.startsWith("ko") ? COPY.ko : COPY.en;

  const navigable = turns.length >= 2;
  const turnIdsRef = useRef<{ ids: readonly string[]; membership: ReadonlySet<string> }>({ ids: [], membership: new Set() });
  const scheduleRef = useRef<(() => void) | null>(null);

  useEffect(() => {
    const previous = turnIdsRef.current.ids;
    if (previous.length === turns.length && turns.every((turn, index) => turn.id === previous[index])) return;
    const ids = turns.map((turn) => turn.id);
    turnIdsRef.current = { ids, membership: new Set(ids) };
    scheduleRef.current?.();
  }, [turns]);

  useEffect(() => {
    const list = listRef.current;
    if (!list || !navigable) {
      setActive("");
      setPreview(null);
      return;
    }
    let frame: number | null = null;
    function update() {
      if (!list) return;
      const { ids, membership } = turnIdsRef.current;
      const boundary = list.getBoundingClientRect().top + 40;
      let current = ids[0] ?? "";
      for (const element of list.querySelectorAll<HTMLElement>("[data-message-id]")) {
        const id = element.dataset.messageId;
        if (id && membership.has(id) && element.getBoundingClientRect().top <= boundary) current = id;
      }
      if (list.scrollHeight > list.clientHeight && list.scrollHeight - list.clientHeight - list.scrollTop < 4) current = ids.at(-1) ?? current;
      setActive(current);
    }
    function schedule() {
      if (frame !== null) return;
      frame = requestAnimationFrame(() => {
        frame = null;
        update();
      });
    }
    scheduleRef.current = schedule;
    update();
    list.addEventListener("scroll", schedule, { passive: true });
    const observer = new ResizeObserver(schedule);
    observer.observe(list);
    const observed = new Set<Element>(list.children);
    for (const child of observed) observer.observe(child);
    // Body changes resize their row; only direct row additions/removals change subscriptions.
    const mutations = new MutationObserver((records) => {
      const changed = new Set<Element>();
      for (const record of records) {
        for (const node of record.addedNodes) if (node instanceof Element) changed.add(node);
        for (const node of record.removedNodes) if (node instanceof Element) changed.add(node);
      }
      for (const child of changed) {
        if (child.parentElement === list) {
          if (!observed.has(child)) {
            observed.add(child);
            observer.observe(child);
          }
        } else if (observed.delete(child)) observer.unobserve(child);
      }
      if (changed.size > 0) schedule();
    });
    mutations.observe(list, { childList: true });
    return () => {
      scheduleRef.current = null;
      list.removeEventListener("scroll", schedule);
      mutations.disconnect();
      observer.disconnect();
      if (frame !== null) cancelAnimationFrame(frame);
    };
  }, [navigable, listRef]);

  useEffect(() => {
    const rail = railRef.current;
    const button = rail?.querySelector<HTMLElement>('[aria-current="step"]');
    if (!rail || !button) return;
    const top = button.offsetTop;
    if (top < rail.scrollTop) rail.scrollTop = top;
    else if (top + button.offsetHeight > rail.scrollTop + rail.clientHeight) rail.scrollTop = top + button.offsetHeight - rail.clientHeight;
  }, [active]);

  if (turns.length < 2) return null;
  const selected = turns.find((turn) => turn.id === preview?.id);
  function showPreview(id: string, button: HTMLElement) {
    const container = listRef.current?.parentElement;
    if (!container) return;
    const top = button.getBoundingClientRect().top - container.getBoundingClientRect().top;
    setPreview({ id, top: Math.max(8, Math.min(top - 24, container.clientHeight - 140)) });
  }
  function jump(id: string) {
    const list = listRef.current;
    const element = Array.from(list?.querySelectorAll<HTMLElement>("[data-message-id]") ?? []).find((item) => item.dataset.messageId === id);
    if (!list || !element) return;
    onNavigate();
    list.scrollTo({ top: list.scrollTop + element.getBoundingClientRect().top - list.getBoundingClientRect().top - 12, behavior: "instant" });
    setActive(id);
    setPreview(null);
  }
  return (
    <div className="chat-navigation" onMouseLeave={() => setPreview(null)}
      onBlur={(event) => { if (!event.currentTarget.contains(event.relatedTarget)) setPreview(null); }}
      onKeyDown={(event) => { if (event.key === "Escape") setPreview(null); }}>
      <nav className="chat-navigation-rail" aria-label={copy.nav} ref={railRef}>
        {turns.map((turn, index) => (
          <button type="button" key={turn.id} aria-label={`${copy.turn(index + 1)}：${turn.title || copy.attachment}`}
            aria-current={active === turn.id ? "step" : undefined}
            aria-describedby={selected?.id === turn.id ? tooltipId : undefined}
            onMouseEnter={(event) => showPreview(turn.id, event.currentTarget)}
            onFocus={(event) => showPreview(turn.id, event.currentTarget)}
            onClick={() => jump(turn.id)}>
            <span aria-hidden="true" />
          </button>
        ))}
      </nav>
      {selected && preview && <div id={tooltipId} role="tooltip" className="chat-navigation-preview" style={{ top: preview.top }}>
        <strong>{selected.title || copy.attachment}</strong>
        <p>{selected.summary || copy.waiting}</p>
      </div>}
    </div>
  );
}
