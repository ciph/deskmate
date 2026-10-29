import { useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { flushSync } from "react-dom";
import { useVirtualizer } from "@tanstack/react-virtual";
import type { Range } from "@tanstack/react-virtual";
import { retainedRange } from "./timelineRange";
import { markdownScheduler } from "./markdownScheduler";
import "./chat-timeline.css";

export interface TimelineControl { jump(index: number): void }
interface Props {
  readonly ids: readonly string[];
  readonly listRef: RefObject<HTMLDivElement | null>;
  readonly controlRef: RefObject<TimelineControl | null>;
  readonly followRef: RefObject<boolean>;
  readonly renderRow: (index: number) => ReactNode;
  readonly onVisible: (index: number) => void;
  readonly geometry: string;
}
const rowOf = (node: Node | null) => (node instanceof Element ? node : node?.parentElement)?.closest<HTMLElement>("[data-timeline-index]");
const rowIndex = (node: Node | null) => { const row = rowOf(node); return row ? Number(row.dataset.timelineIndex) : null; };
const focusables = "button,a[href],input,textarea,select,[tabindex='0'],audio[controls],video[controls]";

function TimelineRow({ index, count, id, start, measure, horizontal, children }: {
  index: number; count: number; id: string; start: number; measure: (node: HTMLLIElement | null) => void;
  horizontal: Map<string, number[]>; children: ReactNode;
}) {
  const ref = useRef<HTMLLIElement>(null);
  const measureRef = useCallback((node: HTMLLIElement | null) => { ref.current = node; measure(node); }, [measure]);
  useLayoutEffect(() => {
    const node = ref.current!;
    const restored = new WeakSet<Element>();
    function restore() { node.querySelectorAll<HTMLElement>("pre,.chat-markdown-table").forEach((element, i) => {
      if (!restored.has(element)) { element.scrollLeft = horizontal.get(id)?.[i] ?? 0; restored.add(element); }
    }); }
    restore();
    const observer = new MutationObserver(restore);
    observer.observe(node, { childList: true, subtree: true });
    return () => { observer.disconnect(); horizontal.set(id, [...node.querySelectorAll<HTMLElement>("pre,.chat-markdown-table")].map(element => element.scrollLeft)); };
  }, [horizontal, id]);
  return <li ref={measureRef} data-index={index} data-timeline-index={index}
    data-timeline-id={id} aria-posinset={index + 1} aria-setsize={count} className="chat-virtual-row" style={{ transform: `translateY(${start}px)` }}>{children}</li>;
}

export function ChatTimeline({ ids, listRef, controlRef, followRef, renderRow, onVisible, geometry }: Props) {
  useEffect(() => () => markdownScheduler.clearCache(), []);
  const [pinIds, setPins] = useState<ReadonlySet<string>>(new Set());
  const [selectionIds, setSelection] = useState<readonly [string, string] | null>(null);
  const [selectAll, setSelectAll] = useState(false);
  const playing = useRef(new Set<HTMLMediaElement>());
  const horizontal = useRef(new Map<string, number[]>());
  const frame = useRef(0), lastVisible = useRef(-1);
  const positions = useMemo(() => new Map(ids.map((id, index) => [id, index])), [ids]);
  const pins = useMemo(() => new Set([...pinIds].flatMap(id => positions.has(id) ? [positions.get(id)!] : [])), [pinIds, positions]);
  const selection = useMemo<readonly [number, number] | null>(() => {
    if (!selectionIds) return null;
    const a = positions.get(selectionIds[0]), b = positions.get(selectionIds[1]);
    return a === undefined || b === undefined ? null : [Math.min(a, b), Math.max(a, b)];
  }, [selectionIds, positions]);
  const getItemKey = useCallback((index: number) => ids[index]!, [ids]);
  const rangeExtractor = useCallback((range: Range) => retainedRange(range, pins, selection, selectAll), [pins, selection, selectAll]);
  const virtual = useVirtualizer<HTMLDivElement, HTMLLIElement>({ count: ids.length, getScrollElement: () => listRef.current,
    getItemKey, estimateSize: () => 180, overscan: 8, rangeExtractor,
    initialRect: { width: 420, height: 500 },
    onChange(instance, sync) {
      const index = instance.range?.startIndex ?? 0;
      if (index !== lastVisible.current) { lastVisible.current = index; onVisible(index); }
      if (!sync && followRef.current) {
        cancelAnimationFrame(frame.current);
        frame.current = requestAnimationFrame(() => { if (followRef.current) instance.scrollToIndex(ids.length - 1, { align: "end" }); });
      }
    },
  });
  virtual.shouldAdjustScrollPositionOnItemSizeChange = (item, _delta, instance) => item.end <= (instance.scrollOffset ?? 0);
  useLayoutEffect(() => {
    controlRef.current = { jump(index) { followRef.current = false; virtual.scrollToIndex(index, { align: "start" }); } };
    return () => { controlRef.current = null; };
  }, [controlRef, followRef, virtual]);
  useLayoutEffect(() => { if (followRef.current && ids.length) virtual.scrollToIndex(ids.length - 1, { align: "end" }); }, [ids.length, followRef, virtual]);
  useEffect(() => {
    const list = listRef.current;
    if (!list) return;
    let width = list.clientWidth;
    const observer = new ResizeObserver(() => {
      if (width === list.clientWidth) return;
      width = list.clientWidth;
      // Keep measured rows until their observer supplies the new width/height;
      // resetting the whole estimate table would move the reading anchor.
      for (const row of list.querySelectorAll<HTMLLIElement>("[data-index]")) virtual.measureElement(row);
    });
    observer.observe(list);
    function selected() {
      const current = document.getSelection();
      if (!current || current.isCollapsed || !list!.contains(current.anchorNode) || !list!.contains(current.focusNode)) {
        setSelection(null); setSelectAll(false); return;
      }
      followRef.current = false;
      const a = rowOf(current.anchorNode)?.dataset.timelineId, b = rowOf(current.focusNode)?.dataset.timelineId;
      if (a && b) setSelection(previous => previous?.[0] === a && previous[1] === b ? previous : [a, b]);
    }
    document.addEventListener("selectionchange", selected);
    return () => { observer.disconnect(); document.removeEventListener("selectionchange", selected); cancelAnimationFrame(frame.current); };
  }, [listRef, virtual, followRef]);
  useLayoutEffect(() => { for (const row of listRef.current?.querySelectorAll<HTMLLIElement>("[data-index]") ?? []) virtual.measureElement(row); }, [geometry, listRef, virtual]);
  function retain() {
    const next = new Set<string>();
    const focus = rowOf(document.activeElement)?.dataset.timelineId;
    if (focus && listRef.current?.contains(document.activeElement)) next.add(focus);
    for (const media of playing.current) {
      if (!media.isConnected) { playing.current.delete(media); continue; }
      const id = rowOf(media)?.dataset.timelineId; if (id) next.add(id);
    }
    setPins(previous => previous.size === next.size && [...next].every(index => previous.has(index)) ? previous : next);
  }
  return <div className="chat-list chat-virtual-list" ref={listRef} data-logical-rows={ids.length}
    data-retained-rows={pins.size + (selection ? selection[1] - selection[0] + 1 : 0)} data-selection-all={selectAll || undefined}
    onFocusCapture={retain} onBlurCapture={() => queueMicrotask(retain)}
    onPlayCapture={event => { if (event.target instanceof HTMLMediaElement) playing.current.add(event.target); retain(); }}
    onPauseCapture={event => { if (event.target instanceof HTMLMediaElement) playing.current.delete(event.target); retain(); }}
    onEndedCapture={event => { if (event.target instanceof HTMLMediaElement) playing.current.delete(event.target); retain(); }}
    onScroll={event => { const list = event.currentTarget; followRef.current = list.scrollHeight - list.clientHeight - list.scrollTop < 48; }}
    onKeyDown={event => {
      const target = event.target as HTMLElement;
      if (target.closest("input,textarea,[contenteditable='true']")) return;
      if (event.nativeEvent.isTrusted && (event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "a") {
        event.preventDefault(); flushSync(() => setSelectAll(true));
        const range = document.createRange(); range.selectNodeContents(event.currentTarget);
        const selected = document.getSelection(); selected?.removeAllRanges(); selected?.addRange(range);
      }
      if (event.key === "Tab") {
        const row = rowOf(target), index = rowIndex(target);
        if (!row || index === null) return;
        const controls = [...row.querySelectorAll<HTMLElement>(focusables)];
        const boundary = event.shiftKey ? controls[0] : controls.at(-1);
        const next = index + (event.shiftKey ? -1 : 1);
        if (target === boundary && next >= 0 && next < ids.length && !listRef.current?.querySelector(`[data-timeline-index="${next}"]`)) {
          event.preventDefault(); virtual.scrollToIndex(next, { align: "auto" });
          requestAnimationFrame(() => { const targets = listRef.current?.querySelector(`[data-timeline-index="${next}"]`)?.querySelectorAll<HTMLElement>(focusables); (event.shiftKey ? targets?.[targets.length - 1] : targets?.[0])?.focus(); });
        }
      }
    }}>
    <ul className="chat-virtual-space" aria-label="Conversation" style={{ height: virtual.getTotalSize() }}>
      {virtual.getVirtualItems().map(item => <TimelineRow key={item.key} id={ids[item.index]!} index={item.index} count={ids.length} start={item.start}
        measure={virtual.measureElement} horizontal={horizontal.current}>{renderRow(item.index)}</TimelineRow>)}
    </ul>
  </div>;
}
