import { expect, test } from "bun:test";
import { renderToStaticMarkup } from "react-dom/server";
import { Fragment } from "react";
import { jsx, jsxs } from "react/jsx-runtime";
import Markdown, { defaultUrlTransform } from "react-markdown";
import remarkGfm from "remark-gfm";
import { toJsxRuntime } from "hast-util-to-jsx-runtime";
import { chatUrl, parseMarkdown } from "./markdownPipeline";
const cases = [
  "[before][target]\n\n# Middle\n\n[target]: https://example.com/a",
  "Footnote[^one]\n\nLater.\n\n[^one]: **note** with [safe](https://example.test)",
  "- one\n  - nested\n\n    paragraph\n\n| A | B |\n|---|---|\n| x | y |",
  "<script>bad()</script>\n\n[bad](javascript:alert%281%29) ![alt](https://example.test/img) [local](file:///C:/x)",
  "```ts\n" + "const x = 1; // literal\n".repeat(2300),
  "a".repeat(50000),
  "**unterminated [link](https://example",
];
test("worker AST has the same final CommonMark/GFM and safety semantics as the released parser", () => {
  for (const source of cases) {
    const expected = renderToStaticMarkup(<Markdown remarkPlugins={[remarkGfm]} skipHtml urlTransform={url => defaultUrlTransform(chatUrl(url))}>{source}</Markdown>);
    const result = parseMarkdown(source, false);
    const actual = renderToStaticMarkup(toJsxRuntime({ type: "root", children: result.blocks.map(block => block.node) }, { Fragment, jsx, jsxs, ignoreInvalidStyle: true, passKeys: true, passNode: true }));
    expect(actual).toBe(expected);
  }
});
test("late definitions invalidate earlier blocks while unrelated stable blocks retain fingerprints", () => {
  const before = parseMarkdown("[link][ref]\n\nStable paragraph.", true);
  const after = parseMarkdown("[link][ref]\n\nStable paragraph.\n\n[ref]: https://example.test", false);
  expect(before.blocks[0]!.fingerprint).not.toBe(after.blocks[0]!.fingerprint);
  expect(before.blocks[2]!.fingerprint).toBe(after.blocks[2]!.fingerprint);
});
