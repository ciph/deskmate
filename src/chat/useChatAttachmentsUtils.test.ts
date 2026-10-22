import { describe, expect, spyOn, test } from "bun:test";
import { prepareModelReadyImage } from "./attachments";
import { planStageUpload, prepareOpenCodeFilePart } from "./useChatAttachmentsUtils";

const MIB = 1024 * 1024;
const budget = { ordinaryBytes: 0, totalBytes: 0 };
const metadata = { id: "image", name: "cat.png", mime: "image/png", size: 4 };

describe("binary attachment preparation", () => {
  test.each([1, 8, 20])("retains the File ArrayBuffer at %i MiB without a numeric array", async (mib) => {
    const file = new File([new Uint8Array(mib * MIB)], "notes.txt", { type: "text/plain" });
    const bytes = await file.arrayBuffer();
    const read = spyOn(file, "arrayBuffer").mockResolvedValue(bytes);
    try {
      const plan = await planStageUpload(file, "session", budget);
      expect(plan.kind).toBe("upload");
      if (plan.kind !== "upload") throw new Error("upload expected");
      expect(plan.request.bytes).toBe(bytes);
      expect(plan.request.bytes.byteLength).toBe(mib * MIB);
      expect(read).toHaveBeenCalledTimes(1);
    } finally { read.mockRestore(); }
  });

  test("rejects empty, unsupported, and limit+1 files before reading", async () => {
    const files = [
      new File([], "empty.png", { type: "image/png" }),
      new File(["payload"], "fake.bin", { type: "application/octet-stream" }),
      new File(["payload"], "large.png", { type: "image/png" }),
    ];
    Object.defineProperty(files[2], "size", { value: 20 * MIB + 1 });
    for (const file of files) {
      const read = spyOn(file, "arrayBuffer").mockRejectedValue(new Error("must not read"));
      try {
        expect((await planStageUpload(file, "session", budget)).kind).toBe("reject");
        expect(read).not.toHaveBeenCalled();
      } finally { read.mockRestore(); }
    }
  });

  test("reuses the validated image data URL without decoding, Blob encoding, or Base64 encoding", async () => {
    const dataUrl = "data:image/png;base64,iVBORw==";
    const decode = spyOn(globalThis, "atob").mockImplementation(() => { throw new Error("unexpected decode"); });
    const encode = spyOn(globalThis, "btoa").mockImplementation(() => { throw new Error("unexpected encode"); });
    const blobRead = spyOn(Blob.prototype, "arrayBuffer").mockRejectedValue(new Error("unexpected blob conversion"));
    try {
      const part = await prepareOpenCodeFilePart({
        id: metadata.id, fileName: metadata.name, mime: "image/png", size: metadata.size,
        sessionId: "session", kind: "image", status: "ready", dataUrl,
      });
      expect(part).toEqual({ type: "file", filename: "cat.png", mime: "image/png", url: dataUrl });
      expect(decode).not.toHaveBeenCalled();
      expect(encode).not.toHaveBeenCalled();
      expect(blobRead).not.toHaveBeenCalled();
    } finally { decode.mockRestore(); encode.mockRestore(); blobRead.mockRestore(); }
  });

  test.each([1, 8, 20])("validates and reuses a %i MiB encoded image", (mib) => {
    const size = mib * MIB;
    const padding = (3 - size % 3) % 3;
    const dataUrl = `data:image/png;base64,${"A".repeat(Math.ceil(size / 3) * 4 - padding)}${"=".repeat(padding)}`;
    expect(prepareModelReadyImage({ ...metadata, size }, dataUrl).dataUrl).toBe(dataUrl);
  });

  test.each([
    "data:image/jpeg;base64,iVBORw==", "data:image/png;charset=utf-8;base64,iVBORw==",
    "data:image/png;base64,iVBORw", "data:image/png;base64,iVBORw===",
    "data:image/png;base64,iVBORx==", "data:image/png;base64,iVBORw= ",
    "data:image/png;base64,iVBORw__", "data:image/png;base64,iVBORw==\n",
  ])("rejects malformed or MIME-mismatched image data: %s", (dataUrl) => {
    expect(() => prepareModelReadyImage(metadata, dataUrl)).toThrow("Invalid staged image");
  });

  test.each(["AAA\n", "AAA\r", "AAAAAAA\n", "AAAAAAA\r"])("rejects newline replacing a base64 character: %s", (encoded) => {
    expect(() => prepareModelReadyImage({ ...metadata, size: encoded.length / 4 * 3 }, `data:image/png;base64,${encoded}`)).toThrow("Invalid staged image");
  });

  test("rejects metadata size mismatch, unsupported MIME, and cancellation", async () => {
    const dataUrl = "data:image/png;base64,iVBORw==";
    for (const patch of [{ size: 0 }, { size: 3 }, { size: 5 }, { size: 20 * MIB + 1 }, { mime: "image/svg+xml" }]) {
      expect(() => prepareModelReadyImage({ ...metadata, ...patch }, dataUrl)).toThrow("Invalid staged image");
    }
    const controller = new AbortController();
    controller.abort();
    await expect(prepareOpenCodeFilePart({
      id: metadata.id, fileName: metadata.name, mime: "image/png", size: metadata.size,
      sessionId: "session", kind: "image", status: "ready", dataUrl,
    }, controller.signal)).rejects.toThrow();
  });
});
