import { afterEach, expect, test } from "bun:test";
import { cleanup, fireEvent, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { renderMarkdown } from "../testing/renderMarkdown";
import { ChatText } from "./ChatText";
import { parseMarkdown } from "./markdownPipeline";
import { markdownPages, MARKDOWN_PAGE_BLOCKS } from "./markdownPages";
afterEach(cleanup);
test("200k dense Markdown mounts one bounded section, preserves references, footnote navigation and full copy", async () => {
  const source = "Beginning[^note] [reference][end]\n\n" + "paragraph **bold**\n\n".repeat(10500) + "\n[end]: https://example.test\n[^note]: Final footnote";
  const descriptor = Object.getOwnPropertyDescriptor(navigator, "clipboard");
  const user = userEvent.setup();
  let copied = "";
  Object.defineProperty(navigator, "clipboard", { configurable: true, value: { writeText: async (text: string) => { copied = text; } } });
  try {
    const view = await renderMarkdown(<ChatText text={source} />, 5000);
    expect(view.container.querySelectorAll("p").length).toBeLessThanOrEqual(MARKDOWN_PAGE_BLOCKS);
    expect(view.getByRole("link", { name: "reference" }).getAttribute("href")).toBe("https://example.test");
    fireEvent.click(view.getByRole("button", { name: "复制全文" }));
    await waitFor(() => expect(copied).toBe(source));
    (view.container.querySelector('a[href="#user-content-fn-note"]') as HTMLAnchorElement).focus();
    await user.keyboard("{Enter}");
    expect(view.container.textContent).toContain("Final footnote");
    fireEvent.click(view.container.querySelector('a[href="#user-content-fnref-note"]')!);
    expect(view.container.textContent).toContain("Beginning");
    expect(view.container.querySelectorAll("*").length).toBeLessThan(300);
  } finally {
    if (descriptor) Object.defineProperty(navigator, "clipboard", descriptor); else Reflect.deleteProperty(navigator, "clipboard");
  }
}, 20000);
test("large tables and ordered lists are segmented without losing rows or numbering", async () => {
  const table = "|a|b|\n|-|-|\n" + Array.from({ length: 1000 }, (_, n) => "|" + n + "|value|\n").join("");
  const result = parseMarkdown(table, false);
  const tables = result.blocks.filter(block => block.node.type === "element" && block.node.tagName === "table");
  expect(tables.length).toBe(25);
  const view = await renderMarkdown(<ChatText text={table} />);
  expect(view.container.querySelectorAll("tr").length).toBeLessThan(160);
  fireEvent.click(view.getByRole("button", { name: "下一段" }));
  expect(view.container.querySelectorAll("tr").length).toBeLessThan(160);
  const list = parseMarkdown(Array.from({ length: 1000 }, (_, n) => (n + 1) + ". item " + n + "\n").join(""), false);
  const groups = list.blocks.filter(block => block.node.type === "element" && block.node.tagName === "ol");
  expect(groups.length).toBe(25);
  const last = groups.at(-1)!.node;
  expect(last.type === "element" && last.properties.start).toBe(961);
  expect(markdownPages(list.blocks).length).toBeGreaterThan(1);
}, 20000);
test("large plain text and code preserve complete content", async () => {
  const plain = "原文 ".repeat(50000);
  const view = await renderMarkdown(<ChatText text={plain} format="plain" />);
  expect(view.container.textContent).toBe(plain);
  const fence = String.fromCharCode(96).repeat(3);
  await view.rerender(<ChatText text={fence + "\n" + plain + "\n" + fence} />);
  expect(view.container.querySelector("pre code")?.textContent).toBe(plain + "\n");
});
