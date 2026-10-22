import { describe, expect, mock, test } from "bun:test";
import { stageChatAttachment, type AttachmentHost, type StageChatAttachmentRequest, type StagedChatAttachment } from "./attachmentApi";
import { binaryUploadOptions, type BinaryUploadMetadata } from "./binaryUpload";
import { decodeUploadOptions, requireBinaryUpload, wrapUploadResponse } from "./binaryUploadTestHelpers.test";

const MIB = 1024 * 1024;
const identity = { uploadId: "upload-1", scopeKey: "chat:session-1:generation-3" };
type Receipt = { uploadId: string; scopeKey: string; value: StagedChatAttachment };

function request(size = 4, mime: StageChatAttachmentRequest["mime"] = "text/plain"): StageChatAttachmentRequest {
  return { sessionId: "session-1", fileName: mime === "application/x-ncm" ? "song.ncm" : "notes.txt", mime, size, bytes: new ArrayBuffer(size) };
}

function hostWithReceipt(rewrite: (receipt: Receipt) => unknown = value => value) {
  const invoke = mock<AttachmentHost["invoke"]>(async (command, body, options) => {
    if (command === "discard_chat_attachment") return { discarded: true };
    if (command !== "stage_chat_attachment_binary") throw new Error(`unexpected ${command}`);
    const metadata = requireBinaryUpload(body, options);
    if (!metadata.sessionId) throw new Error("attachment session missing");
    const value: StagedChatAttachment = {
      id: "returned-attachment", sessionId: metadata.sessionId, fileName: metadata.fileName,
      mime: metadata.mime as StagedChatAttachment["mime"], size: metadata.size,
      kind: metadata.mime === "application/x-ncm" ? "audio" : "text", status: "staged",
    };
    return rewrite(wrapUploadResponse(options, value));
  });
  return { invoke };
}

describe("attachment raw binary uploads", () => {
  test.each([1, 8, 20])("passes the same %i MiB ordinary buffer without JSON byte arrays", async mib => {
    const source = request(mib * MIB);
    const bytes = new Uint8Array(source.bytes);
    bytes[0] = 19;
    bytes[bytes.length - 1] = 253;
    const host = hostWithReceipt();
    const result = await stageChatAttachment(source, host, identity);
    expect(host.invoke).toHaveBeenCalledTimes(1);
    const [command, body, options] = host.invoke.mock.calls[0];
    expect(command).toBe("stage_chat_attachment_binary");
    expect(body).toBe(source.bytes);
    const upload = requireBinaryUpload(body, options);
    expect(upload.bytes.byteLength).toBe(mib * MIB);
    expect(new Uint8Array(upload.bytes)[0]).toBe(19);
    expect(new Uint8Array(upload.bytes)[upload.bytes.byteLength - 1]).toBe(253);
    expect(decodeUploadOptions(options)).toEqual({ ...identity, sessionId: source.sessionId, fileName: source.fileName, mime: source.mime, size: source.size });
    expect(result.size).toBe(source.size);
  });

  test("accepts the 64 MiB NCM boundary as a single raw buffer", async () => {
    const source = request(64 * MIB, "application/x-ncm");
    const host = hostWithReceipt();
    const result = await stageChatAttachment(source, host, identity);
    expect(host.invoke).toHaveBeenCalledTimes(1);
    expect(host.invoke.mock.calls[0][1]).toBe(source.bytes);
    const upload = requireBinaryUpload(host.invoke.mock.calls[0][1], host.invoke.mock.calls[0][2]);
    expect(upload.bytes.byteLength).toBe(64 * MIB);
    expect(result).toMatchObject({ size: 64 * MIB, mime: "application/x-ncm", kind: "audio" });
  });

  test("encodes UTF-8 filename metadata independently from unchanged attachment bytes", async () => {
    const source = { ...request(), fileName: "笔记 🐾.txt" };
    const host = hostWithReceipt();
    const result = await stageChatAttachment(source, host, identity);
    const [, body, options] = host.invoke.mock.calls[0];
    const header = new Headers(options?.headers).get("x-yume-upload");
    expect(header).toMatch(/^[A-Za-z0-9+/]+={0,2}$/);
    expect(decodeUploadOptions(options).fileName).toBe("笔记 🐾.txt");
    expect(decodeUploadOptions(options)).not.toHaveProperty("bytes");
    expect(body).toBe(source.bytes);
    expect(result.fileName).toBe(source.fileName);
  });

  test.each([
    ["empty", () => request(0)],
    ["declared size differs from body", () => ({ ...request(4), size: 3 })],
    ["ordinary limit plus one byte", () => request(20 * MIB + 1)],
    ["NCM limit plus one byte", () => request(64 * MIB + 1, "application/x-ncm")],
    ["JSON number array", () => ({ ...request(4), bytes: [1, 2, 3, 4] })],
    ["typed view instead of raw body", () => ({ ...request(4), bytes: new Uint8Array([1, 2, 3, 4]) })],
  ] as const)("rejects %s before invoking the host", async (_label, makeRequest) => {
    const host = hostWithReceipt();
    await expect(stageChatAttachment(makeRequest() as StageChatAttachmentRequest, host, identity)).rejects.toThrow();
    expect(host.invoke).not.toHaveBeenCalled();
  });

  test.each([
    ["uploadId", (receipt: Receipt) => ({ ...receipt, uploadId: "other-upload" }), "upload response identity mismatch"],
    ["scopeKey", (receipt: Receipt) => ({ ...receipt, scopeKey: "other-generation" }), "upload response identity mismatch"],
    ["sessionId", (receipt: Receipt) => ({ ...receipt, value: { ...receipt.value, sessionId: "unrelated-session" } }), "staged attachment metadata mismatch"],
    ["size", (receipt: Receipt) => ({ ...receipt, value: { ...receipt.value, size: receipt.value.size + 1 } }), "staged attachment metadata mismatch"],
    ["fileName", (receipt: Receipt) => ({ ...receipt, value: { ...receipt.value, fileName: "other.txt" } }), "staged attachment metadata mismatch"],
    ["mime", (receipt: Receipt) => ({ ...receipt, value: { ...receipt.value, mime: "image/png" } }), "staged attachment metadata mismatch"],
  ] as const)("rejects a mismatched %s receipt and discards its ID in the original session", async (_field, rewrite, error) => {
    const source = request();
    const host = hostWithReceipt(rewrite);
    await expect(stageChatAttachment(source, host, identity)).rejects.toThrow(error);
    expect(host.invoke).toHaveBeenCalledTimes(2);
    expect(host.invoke.mock.calls[1]).toEqual([
      "discard_chat_attachment", { request: { sessionId: source.sessionId, attachmentId: "returned-attachment" } },
    ]);
  });
});

describe("binary upload metadata limits", () => {
  const metadata: BinaryUploadMetadata = { ...identity, sessionId: "session-1", fileName: "notes.txt", mime: "text/plain", size: 4 };

  test("accepts identity bounds and the exact 6144 UTF-8 byte metadata boundary", () => {
    const atIdentityBounds = { ...metadata, uploadId: "a".repeat(128), scopeKey: "界".repeat(341) + "a" };
    expect(decodeUploadOptions(binaryUploadOptions(atIdentityBounds))).toEqual(atIdentityBounds);
    const emptyName = { ...metadata, fileName: "" };
    const spare = 6144 - new TextEncoder().encode(JSON.stringify(emptyName)).byteLength;
    const exact = { ...emptyName, fileName: "a".repeat(spare) };
    expect(new TextEncoder().encode(JSON.stringify(exact)).byteLength).toBe(6144);
    expect(decodeUploadOptions(binaryUploadOptions(exact))).toEqual(exact);
    expect(() => binaryUploadOptions({ ...exact, fileName: exact.fileName + "a" })).toThrow("upload metadata too large");
    expect(() => binaryUploadOptions({ ...metadata, fileName: "文".repeat(2048) })).toThrow("upload metadata too large");
  });

  test.each([
    ["empty upload ID", { uploadId: "" }],
    ["upload ID exceeds 128 characters", { uploadId: "a".repeat(129) }],
    ["invalid upload ID character", { uploadId: "upload/id" }],
    ["upload ID trailing newline", { uploadId: "upload\n" }],
    ["empty scope", { scopeKey: "" }],
    ["blank scope", { scopeKey: "   " }],
    ["scope exceeds UTF8 byte limit", { scopeKey: "界".repeat(342) }],
    ["scope contains C1 control", { scopeKey: "scope\u0085other" }],
    ["scope exceeds 1024 characters", { scopeKey: "a".repeat(1025) }],
    ["scope contains a newline", { scopeKey: "scope\nother" }],
    ["scope contains DEL", { scopeKey: "scope\u007fother" }],
  ] as const)("rejects %s", (_label, change) => {
    expect(() => binaryUploadOptions({ ...metadata, ...change })).toThrow("invalid upload identity");
  });
});
