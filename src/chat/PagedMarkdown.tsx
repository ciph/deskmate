import { useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { MarkdownAnchorContext } from "./markdownAnchor";
import type { MarkdownBlock } from "./markdownPipeline";
import { containsMarkdownId, markdownPages } from "./markdownPages";
const TEXT = {
  "zh-CN": ["上一段", "下一段", "复制全文", "已复制", "复制失败，请重试"],
  en: ["Previous section", "Next section", "Copy all", "Copied", "Copy failed. Retry."],
  ja: ["前の部分", "次の部分", "全文をコピー", "コピーしました", "コピー失敗。再試行してください"],
  ko: ["이전 부분", "다음 부분", "전체 복사", "복사됨", "복사 실패. 다시 시도하세요"],
} as const;
export function PagedMarkdown({ blocks, text, streaming, language, renderBlock }: {
  readonly blocks: readonly MarkdownBlock[]; readonly text: string; readonly streaming: boolean;
  readonly language: keyof typeof TEXT; readonly renderBlock: (block: MarkdownBlock) => ReactNode;
}) {
  const pages = useMemo(() => markdownPages(blocks), [blocks]);
  const [selected, setSelected] = useState<number | "tail">(streaming ? "tail" : 0);
  const [feedback, setFeedback] = useState("");
  const root = useRef<HTMLDivElement>(null);
  const anchor = useRef<string | null>(null);
  const page = selected === "tail" ? pages.length - 1 : Math.min(selected, pages.length - 1);
  const labels = TEXT[language];
  useEffect(() => {
    if (!anchor.current) return;
    Array.from(root.current?.querySelectorAll("[id]") ?? []).find(element => element.id === anchor.current)?.scrollIntoView?.();
    anchor.current = null;
  }, [page]);
  const jump = useCallback((href: string): boolean => {
    if (!href.startsWith("#")) return false;
    let id: string;
    try { id = decodeURIComponent(href.slice(1)); } catch { return false; }
    const target = pages.findIndex(section => section.some(block => containsMarkdownId(block.node, id)));
    if (target >= 0 && target !== page) { anchor.current = id; setSelected(target); return true; }
    return false;
  }, [page, pages]);
  async function copyAll() {
    try { await navigator.clipboard.writeText(text); setFeedback(labels[3]); }
    catch { setFeedback(labels[4]); }
  }
  return <MarkdownAnchorContext.Provider value={jump}><div ref={root}>
    {pages.length > 1 && <nav className="chat-markdown-pages" aria-label={language === "zh-CN" ? "长文本分段" : "Document sections"}>
      <button type="button" disabled={page <= 0} onClick={() => setSelected(page - 1)}>{labels[0]}</button>
      <span aria-live="polite">{page + 1} / {pages.length}</span>
      <button type="button" disabled={page >= pages.length - 1} onClick={() => setSelected(page + 1 === pages.length - 1 ? "tail" : page + 1)}>{labels[1]}</button>
      <button type="button" onClick={() => { void copyAll(); }}>{labels[2]}</button>
      <span role="status">{feedback}</span>
    </nav>}
    {pages[page]?.map(renderBlock)}
  </div></MarkdownAnchorContext.Provider>;
}
