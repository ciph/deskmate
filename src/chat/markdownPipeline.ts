import { unified } from "unified";
import remarkParse from "remark-parse";
import remarkGfm from "remark-gfm";
import remarkRehype from "remark-rehype";
import remend from "remend";
import { urlAttributes } from "html-url-attributes";
import type { Root, RootContent, Element } from "hast";

export interface MarkdownBlock { key: string; fingerprint: string; node: RootContent }
export interface MarkdownResult { blocks: MarkdownBlock[]; bytes: number; parseMs: number }
const processor = unified().use(remarkParse).use(remarkGfm).use(remarkRehype, { allowDangerousHtml: true });

export function chatUrl(url: string): string {
  return /^(https?:\/\/|mailto:|#)/iu.test(url) ? url : "";
}
function safeTree(parent: Root | Element) {
  parent.children = parent.children.filter(node => (node as { type: string }).type !== "raw") as typeof parent.children;
  for (const node of parent.children) {
    if (node.type !== "element") continue;
    for (const [key, tags] of Object.entries(urlAttributes)) {
      if (key in node.properties && (!tags || tags.includes(node.tagName))) {
        node.properties[key] = chatUrl(String(node.properties[key] ?? ""));
      }
    }
    safeTree(node);
  }
}
function fingerprint(value: string) {
  let a = 2166136261, b = 5381;
  for (let index = 0; index < value.length; index += 1) {
    a = Math.imul(a ^ value.charCodeAt(index), 16777619);
    b = Math.imul(b, 33) ^ value.charCodeAt(index);
  }
  return `${value.length}:${a >>> 0}:${b >>> 0}`;
}
/** Worker-only full-document parser: global references/footnotes stay global. */
export function parseMarkdown(text: string, streaming: boolean): MarkdownResult {
  const start = performance.now();
  const source = streaming ? remend(text, { katex: false, inlineKatex: false }) : text;
  const tree = processor.runSync(processor.parse(source)) as Root;
  safeTree(tree);
  let bytes = text.length * 2;
  const blocks = tree.children.map((node, index) => {
    const encoded = JSON.stringify(node);
    bytes += encoded.length * 2;
    return { key: `${node.position?.start.offset ?? index}:${node.type === "element" ? node.tagName : node.type}`, fingerprint: fingerprint(encoded), node };
  });
  return { blocks, bytes, parseMs: performance.now() - start };
}
