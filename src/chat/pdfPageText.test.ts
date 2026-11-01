import { afterEach, beforeEach, expect, mock, test } from "bun:test";
import { readPdfPageText } from "./pdfPageText";

const iterator = Object.getOwnPropertyDescriptor(ReadableStream.prototype, Symbol.asyncIterator)!;
beforeEach(() => { Reflect.set(ReadableStream.prototype, Symbol.asyncIterator, undefined); });
afterEach(() => { Object.defineProperty(ReadableStream.prototype, Symbol.asyncIterator, iterator); });

const item = (str: string) => ({ str, dir: "ltr", transform: [], width: 1, height: 1, fontName: "fixture", hasEOL: false });
const text = (str: string) => ({ items: [item(str)], styles: {}, lang: null });
function page(stream: ReadableStream) {
  return {
    isPureXfa: false,
    getTextContent: mock(async () => { throw new Error("async iteration unavailable"); }),
    streamTextContent: () => stream,
  };
}

test("combines streamed chunks and releases the reader without async iteration", async () => {
  const stream = new ReadableStream({ start(controller) {
    controller.enqueue({ items: [item("first"), { type: "beginMarkedContent" }, item("")] });
    controller.enqueue(text("second"));
    controller.close();
  } });
  const source = page(stream);
  expect(await readPdfPageText(source)).toBe("first second");
  expect(source.getTextContent).not.toHaveBeenCalled();
  expect(stream.locked).toBe(false);
});

test("releases the reader and preserves extraction failure", async () => {
  const stream = new ReadableStream({ start(controller) { controller.error(new Error("broken page")); } });
  await expect(readPdfPageText(page(stream))).rejects.toThrow("broken page");
  expect(stream.locked).toBe(false);
});

test("cancellation unblocks a pending read and releases its lock", async () => {
  const cancel = mock(() => {});
  const stream = new ReadableStream({ cancel });
  const signal = new AbortController();
  const result = readPdfPageText(page(stream), signal.signal);
  signal.abort(new Error("cancel fixture"));
  await expect(result).rejects.toThrow("cancel fixture");
  expect(cancel).toHaveBeenCalledTimes(1);
  expect(stream.locked).toBe(false);
});

test("keeps PDF.js XFA extraction when stream iteration is unavailable", async () => {
  const streamTextContent = mock(() => { throw new Error("XFA must not stream"); });
  expect(await readPdfPageText({ isPureXfa: true, getTextContent: async () => text("xfa"), streamTextContent })).toBe("xfa");
  expect(streamTextContent).not.toHaveBeenCalled();
});
