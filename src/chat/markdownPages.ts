import type { Element, RootContent } from "hast";
import type { MarkdownBlock } from "./markdownPipeline";
export const MARKDOWN_PAGE_NODES = 800;
export const MARKDOWN_PAGE_BLOCKS = 80;
export const MARKDOWN_GROUP_ROWS = 40;

/** Keep document-wide reference resolution; split only repeatable structural rows. */
export function splitMarkdownNode(node: RootContent): RootContent[] {
  if (node.type !== "element") return [node];
  const tableBody = node.tagName === "table" ? node.children.find(child => child.type === "element" && child.tagName === "tbody") as Element | undefined : undefined;
  const group = tableBody ?? (node.tagName === "ul" || node.tagName === "ol" ? node : undefined);
  if (!group) return [node];
  const items = group.children.filter(child => child.type === "element");
  if (items.length <= MARKDOWN_GROUP_ROWS) return [node];
  const fragments: RootContent[] = [];
  for (let offset = 0; offset < items.length; offset += MARKDOWN_GROUP_ROWS) {
    const part = { ...group, properties: { ...group.properties }, children: items.slice(offset, offset + MARKDOWN_GROUP_ROWS) };
    if (group.tagName === "ol") part.properties.start = Number(group.properties.start ?? 1) + offset;
    fragments.push(tableBody ? { ...node, children: node.children.map(child => child === tableBody ? part : child) } : part);
  }
  return fragments;
}
export function markdownNodeCount(node: RootContent): number {
  return 1 + (node.type === "element" ? node.children.reduce((count, child) => count + markdownNodeCount(child), 0) : 0);
}
export function markdownPages(blocks: readonly MarkdownBlock[]): readonly (readonly MarkdownBlock[])[] {
  const pages: MarkdownBlock[][] = [];
  let page: MarkdownBlock[] = [], size = 0;
  for (const block of blocks) {
    const count = block.nodes ?? markdownNodeCount(block.node);
    if (page.length && (page.length >= MARKDOWN_PAGE_BLOCKS || size + count > MARKDOWN_PAGE_NODES)) {
      pages.push(page); page = []; size = 0;
    }
    page.push(block); size += count;
  }
  if (page.length) pages.push(page);
  return pages;
}
export function containsMarkdownId(node: RootContent, id: string): boolean {
  return node.type === "element" && (node.properties.id === id || node.children.some(child => containsMarkdownId(child, id)));
}
