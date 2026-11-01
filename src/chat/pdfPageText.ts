import type { PDFPageProxy } from "pdfjs-dist";

type TextItems = Awaited<ReturnType<PDFPageProxy["getTextContent"]>>["items"];
type PdfTextPage = Pick<PDFPageProxy, "isPureXfa" | "getTextContent" | "streamTextContent">;

function strings(items: TextItems): string[] {
  return items.map((item) => ("str" in item ? item.str : "")).filter(Boolean);
}

export async function readPdfPageText(page: PdfTextPage, signal?: AbortSignal): Promise<string> {
  signal?.throwIfAborted();
  // PDF.js 6 uses for-await on ReadableStream. Some native WebKit builds
  // support readers but not async iteration. Keep its separate XFA path.
  if (page.isPureXfa || typeof Reflect.get(ReadableStream.prototype, Symbol.asyncIterator) === "function") {
    const content = await page.getTextContent();
    signal?.throwIfAborted();
    return strings(content.items).join(" ");
  }

  const reader = page.streamTextContent().getReader();
  const parts: string[] = [];
  const abort = () => { void reader.cancel(signal?.reason).catch(() => {}); };
  signal?.addEventListener("abort", abort, { once: true });
  try {
    while (true) {
      signal?.throwIfAborted();
      const { done, value } = await reader.read();
      signal?.throwIfAborted();
      if (done) return parts.join(" ");
      parts.push(...strings(value.items));
    }
  } finally {
    signal?.removeEventListener("abort", abort);
    reader.releaseLock();
  }
}
