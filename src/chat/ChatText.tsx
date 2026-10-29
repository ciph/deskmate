import { createContext, Fragment, memo, useContext, useEffect, useRef, useState } from "react";
import { jsx, jsxs } from "react/jsx-runtime";
import { invoke, isTauri } from "@tauri-apps/api/core";
import type { Components } from "react-markdown";
import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import type { MarkdownBlock } from "./markdownPipeline";
import { markdownScheduler, type MarkdownSnapshot } from "./markdownScheduler";
import { Code } from "@phosphor-icons/react/dist/csr/Code";
import { Copy } from "@phosphor-icons/react/dist/csr/Copy";
import { Check } from "@phosphor-icons/react/dist/csr/Check";
import { WarningCircle } from "@phosphor-icons/react/dist/csr/WarningCircle";
import "./chat-markdown.css";

interface ChatTextProps {
  readonly text: string;
  readonly format?: "plain" | "markdown";
  readonly streaming?: boolean;
  readonly lang?: string;
}

const LABELS = {
  "zh-CN": { code: "代码（可横向滚动）", table: "表格（可横向滚动）", linkError: "无法打开，请复制链接" },
  en: { code: "Code (scroll horizontally)", table: "Table (scroll horizontally)", linkError: "Could not open; copy the link" },
  ja: { code: "コード（横スクロール可能）", table: "表（横スクロール可能）", linkError: "開けません。リンクをコピーしてください" },
  ko: { code: "코드 (가로 스크롤 가능)", table: "표 (가로 스크롤 가능)", linkError: "열 수 없습니다. 링크를 복사하세요" },
} as const;
const COPY_LABELS = {
  "zh-CN": { button: "复制", accessible: "复制代码", success: "已复制", failure: "复制失败，请重试" },
  en: { button: "Copy", accessible: "Copy code", success: "Copied", failure: "Copy failed. Retry." },
  ja: { button: "コピー", accessible: "コードをコピー", success: "コピーしました", failure: "コピー失敗。再試行してください" },
  ko: { button: "복사", accessible: "코드 복사", success: "복사됨", failure: "복사 실패. 다시 시도하세요" },
} as const;
const CopyLabelsContext = createContext<{ readonly button: string; readonly accessible: string; readonly success: string; readonly failure: string }>(COPY_LABELS["zh-CN"]);
const LabelsContext = createContext<{ readonly code: string; readonly table: string; readonly linkError: string }>(LABELS["zh-CN"]);
const COMPONENTS: Components = {
  a: function MarkdownLink({ href, children, title, id }) {
    const [failed, setFailed] = useState(false);
    const labels = useContext(LabelsContext);
    if (!href) return <span>{children}</span>;
    return (
      <>
      <a href={href} id={id} title={title ?? href}
        target={href.startsWith("#") ? undefined : "_blank"} rel="noopener noreferrer"
        onClick={(event) => {
          if (href.startsWith("#") || !isTauri()) return;
          event.preventDefault();
          setFailed(false);
          void invoke("open_chat_link", { url: href }).catch(() => setFailed(true));
        }}>
        {children}
      </a>
      {failed && <span role="alert"> ({labels.linkError}: {href})</span>}
      </>
    );
  },
  img: function MarkdownImage({ alt }) {
    return <span className="chat-markdown-image-alt">{alt}</span>;
  },
  pre: function MarkdownCode({ children, node }) {
    const labels = useContext(LabelsContext);
    const copyLabels = useContext(CopyLabelsContext);
    const codeRef = useRef<HTMLPreElement>(null);
    const [copying, setCopying] = useState(false);
    const [feedback, setFeedback] = useState("");
    const codeNode = node?.children.find((child) => child.type === "element" && child.tagName === "code");
    const classes = codeNode?.type === "element" ? codeNode.properties.className : undefined;
    const languageClass = Array.isArray(classes) ? classes.find((name) => typeof name === "string" && name.startsWith("language-")) : undefined;
    const language = typeof languageClass === "string" ? languageClass.slice(9) : "text";
    useEffect(() => {
      if (!feedback) return;
      const timer = window.setTimeout(() => setFeedback(""), 2000);
      return () => window.clearTimeout(timer);
    }, [feedback]);
    async function copyCode() {
      if (!codeRef.current || copying) return;
      const text = codeRef.current.textContent ?? "";
      setCopying(true);
      setFeedback("");
      try {
        await navigator.clipboard.writeText(text);
        setFeedback(copyLabels.success);
      } catch (error) {
        setFeedback(copyLabels.failure);
        if (!(error instanceof Error)) console.error("Clipboard write failed", error);
      } finally {
        setCopying(false);
      }
    }
    return (
      <div className="chat-code-block">
        <div className="chat-code-toolbar">
          <span className="chat-code-language"><Code size={16} aria-hidden="true" /><span>{language}</span></span>
          <span className="chat-code-status" role="status">{feedback}</span>
          <button type="button" aria-label={copyLabels.accessible} title={feedback || copyLabels.accessible} aria-disabled={copying} onClick={copyCode}>
            {feedback === copyLabels.success ? <Check size={16} aria-hidden="true" /> : feedback === copyLabels.failure ? <WarningCircle size={16} aria-hidden="true" /> : <Copy size={16} aria-hidden="true" />}
          </button>
        </div>
        <pre ref={codeRef} tabIndex={0} role="region" aria-label={labels.code}>{children}</pre>
      </div>
    );
  },
  table: function MarkdownTable({ children }) {
    const labels = useContext(LabelsContext);
    return (
      <div className="chat-markdown-table" tabIndex={0} role="region" aria-label={labels.table}>
        <table>{children}</table>
      </div>
    );
  },
};

const RichBlock = memo(function RichBlock({ block }: { block: MarkdownBlock }) {
  return toJsxRuntime(block.node, { Fragment, jsx, jsxs, components: COMPONENTS, ignoreInvalidStyle: true, passKeys: true, passNode: true });
});
function languageKey(lang: string) {
  if (lang.startsWith("zh")) return "zh-CN";
  if (lang.startsWith("ja")) return "ja";
  if (lang.startsWith("ko")) return "ko";
  return "en";
}
const FORMAT_UNAVAILABLE = {
  "zh-CN": "格式暂不可用，可复制原文", ja: "書式を表示できません。原文はコピーできます",
  ko: "서식을 사용할 수 없습니다. 원문을 복사할 수 있습니다", en: "Formatting unavailable; original text can be copied",
};
function RichText({ text, streaming = false, lang = "zh-CN" }: ChatTextProps) {
  const [snapshot, setSnapshot] = useState<MarkdownSnapshot | null>(null);
  const client = useRef<ReturnType<typeof markdownScheduler.open> | null>(null);
  useEffect(() => {
    client.current = markdownScheduler.open(setSnapshot);
    return () => { client.current?.close(); client.current = null; };
  }, []);
  useEffect(() => { client.current?.update(text, streaming); }, [text, streaming]);
  const language = languageKey(lang);
  const labels = LABELS[language];
  const copyLabels = COPY_LABELS[language];
  const valid = snapshot && !snapshot.unavailable && text.startsWith(snapshot.text) ? snapshot : null;
  const tail = text.slice(valid?.text.length ?? 0);
  return (
    <LabelsContext.Provider value={labels}>
      <CopyLabelsContext.Provider value={copyLabels}>
      <div className="chat-markdown" data-format-pending={!valid || valid.text !== text || valid.streaming !== streaming ? "true" : undefined}>
        {valid?.blocks.map(block => <RichBlock key={block.key} block={block} />)}
        {tail && <span className="chat-plain-text">{tail}</span>}
        {snapshot?.unavailable && <span role="status">{FORMAT_UNAVAILABLE[language]}</span>}
      </div>
      </CopyLabelsContext.Provider>
    </LabelsContext.Provider>
  );
}

export const ChatText = memo(function ChatText(props: ChatTextProps) {
  return props.format === "plain" ? <span className="chat-plain-text">{props.text}</span> : <RichText {...props} />;
});
