import { afterEach, beforeEach, describe, expect, mock, spyOn, test } from "bun:test";
import { act, cleanup, render, waitFor } from "@testing-library/react";
import { createElement } from "react";
import type { AttachmentHost } from "./attachmentApi";
import { useChatAttachments, type UseChatAttachmentsResult } from "./useChatAttachments";
import { createFakeHost, drive, file, rejectStage, resolveStage } from "./useChatAttachmentsHarness.test";

let controller: UseChatAttachmentsResult;
let id = 0;
const created: Blob[] = [];
const revoked: string[] = [];
let createUrl: ReturnType<typeof spyOn<typeof URL, "createObjectURL">>;
let revokeUrl: ReturnType<typeof spyOn<typeof URL, "revokeObjectURL">>;

beforeEach(() => {
  id = 0;
  created.length = 0;
  revoked.length = 0;
  createUrl = spyOn(URL, "createObjectURL").mockImplementation((blob) => {
    created.push(blob as Blob);
    return `blob:preview-${created.length}`;
  });
  revokeUrl = spyOn(URL, "revokeObjectURL").mockImplementation((url) => { revoked.push(url); });
});

afterEach(() => {
  cleanup();
  createUrl.mockRestore();
  revokeUrl.mockRestore();
});

function Harness(props: { host: AttachmentHost; sessionId?: string }) {
  controller = useChatAttachments({
    host: props.host, sessionId: props.sessionId ?? "ses-1", personaId: "xiaozhu",
    makeLocalId: () => `preview-${++id}`,
  });
  return null;
}

function delayedFile() {
  let resolve!: (bytes: ArrayBuffer) => void;
  const bytes = new TextEncoder().encode("GIF89a").buffer;
  const pending = new Promise<ArrayBuffer>((finish) => { resolve = finish; });
  const input = file("cat.gif", "GIF89a", "image/gif");
  Object.defineProperty(input, "arrayBuffer", { value: mock(() => pending) });
  return { input, finish: () => resolve(bytes) };
}

async function readyImage(host: ReturnType<typeof createFakeHost>) {
  await drive(() => controller.stageFromPicker([file("cat.gif", "GIF89a", "image/gif")]));
  await waitFor(() => expect(host.stageRequests).toHaveLength(1));
  await resolveStage(host, 0, "stage-image", "image");
  await waitFor(() => expect(controller.items[0]?.kind).toBe("ready"));
}

describe("attachment preview ownership", () => {
  test.each(["remove", "cancel", "send", "reset", "unmount"])("releases a Blob URL once on %s", async (event) => {
    const host = createFakeHost();
    const view = render(createElement(Harness, { host }));
    await readyImage(host);
    expect(created).toHaveLength(1);
    expect(created[0]?.type).toBe("image/gif");
    expect(new TextDecoder().decode(await created[0]!.arrayBuffer())).toBe("GIF89a");
    expect(controller.items[0]).toMatchObject({ source: { previewDataUrl: "blob:preview-1" } });
    await drive(() => {
      if (event === "send") controller.discardSentSources(["preview-1"]);
      else if (event === "reset") controller.resetSession("ses-2");
      else if (event === "unmount") view.unmount();
      else if (event === "cancel") controller.cancel("preview-1");
      else controller.remove("preview-1");
    });
    expect(revoked).toEqual(["blob:preview-1"]);
    view.unmount();
    expect(revoked).toEqual(["blob:preview-1"]);
  });

  test.each(["remove", "cancel", "reset-return", "unmount"])("does not start IPC when File bytes arrive after %s", async (event) => {
    const host = createFakeHost();
    const view = render(createElement(Harness, { host }));
    const delayed = delayedFile();
    await drive(() => controller.stageFromPicker([delayed.input]));
    await drive(() => {
      if (event === "reset-return") { controller.resetSession("ses-2"); controller.resetSession("ses-1"); }
      else if (event === "unmount") view.unmount();
      else if (event === "cancel") controller.cancel("preview-1");
      else controller.remove("preview-1");
    });
    await drive(delayed.finish);
    expect(host.stageRequests).toHaveLength(0);
    expect(created).toHaveLength(0);
  });

  test.each(["remove", "reset-return", "unmount"])("discards late stage response without creating a preview after %s", async (event) => {
    const host = createFakeHost();
    const view = render(createElement(Harness, { host }));
    await drive(() => controller.stageFromPicker([file("cat.gif", "GIF89a", "image/gif")]));
    await waitFor(() => expect(host.stageRequests).toHaveLength(1));
    await drive(() => {
      if (event === "reset-return") { controller.resetSession("ses-2"); controller.resetSession("ses-1"); }
      else if (event === "unmount") view.unmount();
      else controller.remove("preview-1");
    });
    await resolveStage(host, 0, "late-image", "image");
    expect(host.discardRequests).toEqual([{ sessionId: "ses-1", attachmentId: "late-image" }]);
    expect(created).toHaveLength(0);
  });

  test("retries a failed File read before starting binary IPC", async () => {
    const host = createFakeHost();
    render(createElement(Harness, { host }));
    const input = file("cat.gif", "GIF89a", "image/gif");
    const bytes = await input.arrayBuffer();
    const read = spyOn(input, "arrayBuffer")
      .mockRejectedValueOnce(new Error("transient read error"))
      .mockResolvedValue(bytes);
    try {
      await drive(() => controller.stageFromPicker([input]));
      await waitFor(() => expect(controller.items[0]).toMatchObject({ kind: "failed", phase: "staging" }));
      expect(host.stageRequests).toHaveLength(0);
      await drive(() => controller.retry("preview-1"));
      await waitFor(() => expect(host.stageRequests).toHaveLength(1));
      expect(read).toHaveBeenCalledTimes(2);
      expect(host.stageRequests[0]?.bytes).toBe(bytes);
      await resolveStage(host, 0, "read-retry-image", "image");
      expect(controller.items[0]?.kind).toBe("ready");
      expect(created).toHaveLength(1);
    } finally { read.mockRestore(); }
  });

  test("retains only byte snapshot on failure and creates one URL after a successful retry", async () => {
    const host = createFakeHost();
    render(createElement(Harness, { host }));
    await drive(() => controller.stageFromPicker([file("cat.gif", "GIF89a", "image/gif")]));
    await waitFor(() => expect(host.stageRequests).toHaveLength(1));
    await rejectStage(host, 0, "temporary I/O failure");
    expect(created).toHaveLength(0);
    await drive(() => controller.retry("preview-1"));
    await waitFor(() => expect(host.stageRequests).toHaveLength(2));
    expect(host.stageRequests[1]?.bytes).toBe(host.stageRequests[0]?.bytes);
    await resolveStage(host, 1, "retry-image", "image");
    expect(created).toHaveLength(1);
    await drive(() => controller.remove("preview-1"));
    expect(revoked).toEqual(["blob:preview-1"]);
  });

  test.each([false, true])("releases previews after preparation (failure=%s) while preserving staged retry source", async (fails) => {
    const host = createFakeHost();
    render(createElement(Harness, { host }));
    await readyImage(host);
    host.readResponses.set("stage-image", {
      id: "stage-image", sessionId: "ses-1", fileName: "cat.gif", mime: "image/gif", size: 6,
      kind: "image", status: "ready", dataUrl: fails ? "data:image/gif;base64,broken!" : "data:image/gif;base64,R0lGODlh",
    });
    await act(async () => {
      if (fails) await expect(controller.prepareModelAttachments("")).rejects.toThrow();
      else expect((await controller.prepareModelAttachments("")).fileParts[0]?.url).toBe("data:image/gif;base64,R0lGODlh");
    });
    expect(revoked).toEqual(["blob:preview-1"]);
    const item = controller.items[0];
    expect(item?.kind).toBe("ready");
    if (item?.kind !== "ready") throw new Error("ready item expected");
    expect(item.source.previewDataUrl).toBeUndefined();
    expect(item.source.id).toBe("stage-image");
    expect(host.discardRequests).toHaveLength(0);
  });
});
