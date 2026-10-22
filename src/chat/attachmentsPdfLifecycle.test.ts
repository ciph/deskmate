import { afterEach, beforeEach, describe, expect, mock, test } from "bun:test";
import { cleanup, render, waitFor } from "@testing-library/react";
import { createElement } from "react";
import { prepareModelReadyAttachment } from "./attachments";
import { useChatAttachments, type UseChatAttachmentsResult } from "./useChatAttachments";
import { createFakeHost, drive, file, ready, resolveStage, type FakeHost } from "./useChatAttachmentsHarness.test";

type PdfDocument = {
  numPages: number;
  getPage: (page: number) => Promise<{ getTextContent: () => Promise<{ items: { str: string }[] }> }>;
};
type LoadingTask = { promise: Promise<PdfDocument>; destroy: () => Promise<void> };
let load: () => LoadingTask;
const getDocument = mock(() => load());
const workerOptions = { workerSrc: "" };
mock.module("pdfjs-dist/legacy/build/pdf.mjs", () => ({
  getDocument,
  GlobalWorkerOptions: workerOptions,
}));
mock.module("pdfjs-dist/legacy/build/pdf.worker.min.mjs?url", () => ({ default: "/assets/pdf.worker-fixture.mjs" }));

let controller: UseChatAttachmentsResult | undefined;
beforeEach(() => {
  getDocument.mockClear();
  workerOptions.workerSrc = "";
  load = () => { throw new Error("test loading task missing"); };
});
afterEach(() => {
  cleanup();
  controller = undefined;
});

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (error: Error) => void;
  const promise = new Promise<T>((accept, fail) => { resolve = accept; reject = fail; });
  return { promise, resolve, reject };
}

function pages(texts: string[]) {
  const getPage = mock(async (number: number) => ({
    getTextContent: async () => ({ items: [{ str: texts[number - 1] ?? "" }] }),
  }));
  const destroy = mock(async () => {});
  load = () => ({ promise: Promise.resolve({ numPages: texts.length, getPage }), destroy });
  return { getPage, destroy };
}

function prepare(signal?: AbortSignal) {
  return prepareModelReadyAttachment({
    metadata: { id: "pdf", name: "sample.pdf", mime: "application/pdf", size: 4 },
    bytes: new TextEncoder().encode("%PDF").buffer,
  }, signal);
}

describe("PDF attachment worker lifecycle", () => {
  test("uses the bundled worker URL and awaits destruction after success", async () => {
    const closing = deferred<void>();
    const { destroy } = pages(["page one", "page two"]);
    destroy.mockImplementation(() => closing.promise);
    let settled = false;
    const result = prepare().then((value) => { settled = true; return value; });
    await waitFor(() => expect(destroy).toHaveBeenCalledTimes(1));
    expect(workerOptions.workerSrc).toBe("/assets/pdf.worker-fixture.mjs");
    expect(settled).toBe(false);
    closing.resolve();
    expect(atob((await result).dataUrl.split(",")[1] ?? "")).toBe("page one\n\npage two");
  });

  test("destroys the loading task when the document cannot load", async () => {
    const destroy = mock(async () => {});
    load = () => ({ promise: Promise.reject(new Error("invalid PDF")), destroy });
    await expect(prepare()).rejects.toThrow("invalid PDF");
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  test.each(["getPage", "getTextContent"])("destroys the task after %s fails", async (stage) => {
    const destroy = mock(async () => {});
    load = () => ({
      destroy,
      promise: Promise.resolve({ numPages: 1, getPage: async () => {
        if (stage === "getPage") throw new Error("broken page");
        return { getTextContent: async () => { throw new Error("broken page"); } };
      } }),
    });
    await expect(prepare()).rejects.toThrow("broken page");
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  test("stops extracting after the character limit and destroys unused pages", async () => {
    const { getPage, destroy } = pages(["x".repeat(160_001), "must not read"]);
    const attachment = await prepare();
    expect(attachment).toMatchObject({ truncated: true });
    expect(atob(attachment.dataUrl.split(",")[1] ?? "").startsWith("x".repeat(160_000))).toBe(true);
    expect(getPage).toHaveBeenCalledTimes(1);
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  test("does not silently hide following pages when the first is exactly the limit", async () => {
    const { getPage, destroy } = pages(["x".repeat(160_000), "tail", "must not read"]);
    expect(await prepare()).toMatchObject({ truncated: true });
    expect(getPage).toHaveBeenCalledTimes(2);
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  test("includes page separators in the text limit", async () => {
    const { getPage } = pages(["x".repeat(80_000), "y".repeat(80_000), "must not read"]);
    expect(await prepare()).toMatchObject({ truncated: true });
    expect(getPage).toHaveBeenCalledTimes(2);
  });

  test("does not create a worker for an already cancelled preparation", async () => {
    const cancellation = new AbortController();
    cancellation.abort();
    await expect(prepare(cancellation.signal)).rejects.toMatchObject({ name: "AbortError" });
    expect(getDocument).not.toHaveBeenCalled();
  });

  test("cancels a pending load and awaits its worker cleanup", async () => {
    const loading = deferred<PdfDocument>();
    const closing = deferred<void>();
    const destroy = mock(() => closing.promise);
    load = () => ({ promise: loading.promise, destroy });
    const cancellation = new AbortController();
    const result = prepare(cancellation.signal).catch((error: unknown) => error);
    await waitFor(() => expect(getDocument).toHaveBeenCalledTimes(1));
    cancellation.abort();
    await waitFor(() => expect(destroy).toHaveBeenCalledTimes(1));
    closing.resolve();
    expect(await result).toMatchObject({ name: "AbortError" });
  });

  test("cancels pending page extraction without waiting for the page to finish", async () => {
    const text = deferred<{ items: { str: string }[] }>();
    const getTextContent = mock(() => text.promise);
    const destroy = mock(async () => {});
    load = () => ({
      promise: Promise.resolve({ numPages: 1, getPage: async () => ({ getTextContent }) }), destroy,
    });
    const cancellation = new AbortController();
    const result = prepare(cancellation.signal).catch((error: unknown) => error);
    await waitFor(() => expect(getTextContent).toHaveBeenCalledTimes(1));
    cancellation.abort();
    expect(await result).toMatchObject({ name: "AbortError" });
    expect(destroy).toHaveBeenCalledTimes(1);
  });
});

function Harness({ host }: { host: FakeHost }) {
  controller = useChatAttachments({ sessionId: "ses-1", personaId: "xiaozhu", host, makeLocalId: () => "local-pdf" });
  return null;
}

async function stagePdf(host: FakeHost) {
  await drive(() => controller!.stageFromPicker([file("sample.pdf", "%PDF", "application/pdf")]));
  await waitFor(() => expect(host.stageRequests).toHaveLength(1));
  await resolveStage(host, 0, "stage-pdf", "text");
  host.readResponses.set("stage-pdf", ready("stage-pdf", "sample.pdf", "application/pdf", "text"));
  await waitFor(() => expect(controller!.items[0]?.kind).toBe("ready"));
}

describe("PDF preparation cancellation from the composer", () => {
  test.each(["remove", "cancel", "reset", "unmount"])("%s stops an in-flight PDF", async (action) => {
    const loading = deferred<PdfDocument>();
    const destroy = mock(async () => {});
    load = () => ({ promise: loading.promise, destroy });
    const host = createFakeHost();
    const view = render(createElement(Harness, { host }));
    await stagePdf(host);
    const result = controller!.prepareModelAttachments("summarize").catch((error: unknown) => error);
    await waitFor(() => expect(getDocument).toHaveBeenCalledTimes(1));
    await drive(() => {
      if (action === "remove") controller!.remove("local-pdf");
      else if (action === "cancel") controller!.cancel("local-pdf");
      else if (action === "reset") controller!.resetSession("ses-2");
      else view.unmount();
    });
    expect(await result).toMatchObject({ name: "AbortError" });
    expect(destroy).toHaveBeenCalledTimes(1);
  });

  test("a session reset during backend read never starts a late PDF worker", async () => {
    const host = createFakeHost();
    const read = deferred<ReturnType<typeof ready>>();
    const originalInvoke = host.invoke;
    host.invoke = (command, payload, options) => command === "read_chat_attachment" ? read.promise : originalInvoke(command, payload, options);
    render(createElement(Harness, { host }));
    await stagePdf(host);
    const result = controller!.prepareModelAttachments("summarize").catch((error: unknown) => error);
    await drive(() => controller!.resetSession("ses-2"));
    read.resolve(ready("stage-pdf", "sample.pdf", "application/pdf", "text"));
    expect(await result).toMatchObject({ name: "AbortError" });
    expect(getDocument).not.toHaveBeenCalled();
  });

  test("a superseded preparation cannot cancel the next preparation", async () => {
    const loading = deferred<PdfDocument>();
    const firstDestroy = mock(async () => {});
    const secondDestroy = mock(async () => {});
    let loads = 0;
    load = () => ++loads === 1
      ? { promise: loading.promise, destroy: firstDestroy }
      : { promise: Promise.resolve({ numPages: 1, getPage: async () => ({ getTextContent: async () => ({ items: [{ str: "next" }] }) }) }), destroy: secondDestroy };
    const host = createFakeHost();
    render(createElement(Harness, { host }));
    await stagePdf(host);
    const first = controller!.prepareModelAttachments("first").catch((error: unknown) => error);
    await waitFor(() => expect(getDocument).toHaveBeenCalledTimes(1));
    const second = controller!.prepareModelAttachments("second");
    expect(await first).toMatchObject({ name: "AbortError" });
    expect((await second).fileParts).toHaveLength(1);
    expect(firstDestroy).toHaveBeenCalledTimes(1);
    expect(secondDestroy).toHaveBeenCalledTimes(1);
  });

  test("one failed attachment cancels a sibling PDF still loading", async () => {
    const loading = deferred<PdfDocument>();
    const destroy = mock(async () => {});
    load = () => ({ promise: loading.promise, destroy });
    const host = createFakeHost();
    let nextId = 0;
    const BatchHarness = () => {
      controller = useChatAttachments({ sessionId: "ses-1", personaId: "xiaozhu", host, makeLocalId: () => `batch-${++nextId}` });
      return null;
    };
    render(createElement(BatchHarness));
    await drive(() => controller!.stageFromPicker([file("sample.pdf", "%PDF", "application/pdf"), file("broken.txt", "broken", "text/plain")]));
    await waitFor(() => expect(host.stageRequests).toHaveLength(2));
    await resolveStage(host, 0, "batch-pdf", "text");
    await resolveStage(host, 1, "batch-broken", "text");
    host.readResponses.set("batch-pdf", ready("batch-pdf", "sample.pdf", "application/pdf", "text"));
    const brokenRead = deferred<ReturnType<typeof ready>>();
    const originalInvoke = host.invoke;
    host.invoke = (command, payload, options) => command === "read_chat_attachment" && !(payload instanceof ArrayBuffer) && "attachmentId" in payload.request && payload.request.attachmentId === "batch-broken"
      ? brokenRead.promise : originalInvoke(command, payload, options);
    const result = controller!.prepareModelAttachments("summarize").catch((error: unknown) => error);
    await waitFor(() => expect(getDocument).toHaveBeenCalledTimes(1));
    brokenRead.reject(new Error("attachment missing"));
    expect(await result).toMatchObject({ message: "attachment missing" });
    await waitFor(() => expect(destroy).toHaveBeenCalledTimes(1));
  });
});
