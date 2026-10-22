import { describe, expect, test } from "bun:test";
import type { InvokeOptions } from "@tauri-apps/api/core";
import { decodeUploadOptions, wrapUploadResponse } from "./binaryUploadTestHelpers.test";
import {
  cleanupChatSession,
  convertStagedNcm,
  discardChatAttachment,
  exportChatArtifact,
  readChatAttachment,
  stageChatAttachment,
  type AttachmentCommandName,
  type AttachmentCommandPayload,
  type AttachmentHost,
  type StageChatAttachmentRequest,
} from "./attachmentApi";

const STAGE_REQUEST = {
  sessionId: "ses-1",
  fileName: "large.ncm",
  mime: "application/x-ncm",
  size: 3,
  bytes: new Uint8Array([1, 2, 3]).buffer,
} satisfies StageChatAttachmentRequest;
const STAGED_RESPONSE = {
  id: "att-1",
  sessionId: "ses-1",
  fileName: "large.ncm",
  mime: "application/x-ncm",
  size: 3,
  kind: "audio",
  status: "staged",
};
const READY_RESPONSE = {
  id: "att-1",
  sessionId: "ses-1",
  fileName: "song.mp3",
  mime: "audio/mpeg",
  size: 32,
  kind: "audio",
  status: "ready",
  dataUrl: "data:audio/mpeg;base64,AAAA",
};
const EXPORT_RESPONSE = {
  artifactId: "art-1",
  sessionId: "ses-1",
  fileName: "song.mp3",
  mime: "audio/mpeg",
  size: 32,
  exportedAt: "2026-09-01T10:00:00.000Z",
};

class FakeAttachmentHost implements AttachmentHost {
  readonly calls: Array<{
    readonly command: AttachmentCommandName;
    readonly payload: AttachmentCommandPayload | ArrayBuffer;
    readonly options?: InvokeOptions;
  }> = [];

  constructor(private readonly responses: readonly unknown[]) {}

  async invoke(command: AttachmentCommandName, payload: AttachmentCommandPayload | ArrayBuffer, options?: InvokeOptions): Promise<unknown> {
    this.calls.push({ command, payload, ...(options ? { options } : {}) });
    const response = this.responses[this.calls.length - 1];
    if (response instanceof Error) throw response;
    return command === "stage_chat_attachment_binary" ? wrapUploadResponse(options, response) : response;
  }
}

describe("attachment api native path leak boundaries", () => {
  test("sends captured stage bytes when request bytes accessor swaps later", async () => {
    const firstBytes = new Uint8Array([7, 8, 9]).buffer;
    const swappedBytes: number[] = [];
    Object.defineProperty(swappedBytes, "0", {
      configurable: true,
      enumerable: true,
      value: "C:\\Users\\secret\\large.ncm",
    });
    let bytesReads = 0;
    const request: StageChatAttachmentRequest = {
      ...STAGE_REQUEST,
      get bytes(): ArrayBuffer {
        bytesReads += 1;
        return bytesReads === 1 ? firstBytes : swappedBytes as unknown as ArrayBuffer;
      },
    };
    const host = new FakeAttachmentHost([STAGED_RESPONSE]);

    await expect(stageChatAttachment(request, host)).resolves.toMatchObject({ fileName: "large.ncm" });

    expect(bytesReads).toBe(1);
    expect(host.calls[0]?.payload).toBe(firstBytes);
    expect(decodeUploadOptions(host.calls[0]?.options)).toMatchObject({ sessionId: "ses-1", fileName: "large.ncm", size: 3 });
  });

  test("drops nested request extra fields and accessors before host invocation", async () => {
    const request: StageChatAttachmentRequest & { readonly audit: unknown } = {
      ...STAGE_REQUEST,
      get audit(): unknown {
        throw new Error("extra accessor was read");
      },
    };
    const host = new FakeAttachmentHost([STAGED_RESPONSE]);

    await expect(stageChatAttachment(request, host)).resolves.toMatchObject({ id: "att-1" });

    expect(host.calls[0]?.payload).toBe(STAGE_REQUEST.bytes);
    expect(decodeUploadOptions(host.calls[0]?.options)).not.toHaveProperty("audit");
  });

  test("normalizes accessor-backed ids before invoking every command", async () => {
    const host = new FakeAttachmentHost([
      { ...READY_RESPONSE, fileName: "note.txt", mime: "text/plain", kind: "text", dataUrl: "data:text/plain;base64,aGk=" },
      { discarded: true },
      READY_RESPONSE,
      EXPORT_RESPONSE,
      { removed: 1 },
    ]);
    const readSession = swap("ses-1", "ses-read-2");
    const readAttachment = swap("att-1", "att-read-2");
    const discardSession = swap("ses-1", "ses-discard-2");
    const discardAttachment = swap("att-1", "att-discard-2");
    const convertSession = swap("ses-1", "ses-convert-2");
    const convertAttachment = swap("att-1", "att-convert-2");
    const convertPersona = swap("xiaozhu", "persona-2");
    const exportSession = swap("ses-1", "ses-export-2");
    const exportArtifact = swap("art-1", "art-2");
    const cleanupSession = swap("ses-1", "ses-cleanup-2");

    await readChatAttachment({ get sessionId() { return readSession(); }, get attachmentId() { return readAttachment(); } }, host);
    await discardChatAttachment({ get sessionId() { return discardSession(); }, get attachmentId() { return discardAttachment(); } }, host);
    await convertStagedNcm({ get sessionId() { return convertSession(); }, get attachmentId() { return convertAttachment(); }, get personaId() { return convertPersona(); } }, host);
    await exportChatArtifact({ get sessionId() { return exportSession(); }, get artifactId() { return exportArtifact(); } }, host);
    await cleanupChatSession({ get sessionId() { return cleanupSession(); } }, host);

    expect(host.calls).toEqual([
      { command: "read_chat_attachment", payload: { request: { sessionId: "ses-1", attachmentId: "att-1" } } },
      { command: "discard_chat_attachment", payload: { request: { sessionId: "ses-1", attachmentId: "att-1" } } },
      { command: "convert_staged_ncm", payload: { request: { sessionId: "ses-1", attachmentId: "att-1", personaId: "xiaozhu" } } },
      { command: "export_chat_artifact", payload: { request: { sessionId: "ses-1", artifactId: "art-1" } } },
      { command: "cleanup_chat_session", payload: { request: { sessionId: "ses-1" } } },
    ]);
  });

  test("rejects native path strings in response bytes fields", async () => {
    await expect(stageChatAttachment(STAGE_REQUEST, new FakeAttachmentHost([{ ...STAGED_RESPONSE, bytes: pathBytes("stage.ncm") }]))).rejects.toThrow("native path value leaked");
    await expect(readChatAttachment({ sessionId: "ses-1", attachmentId: "att-1" }, new FakeAttachmentHost([{ ...READY_RESPONSE, bytes: pathBytes("song.mp3") }]))).rejects.toThrow("native path value leaked");
    await expect(exportChatArtifact({ sessionId: "ses-1", artifactId: "art-1" }, new FakeAttachmentHost([{ ...EXPORT_RESPONSE, bytes: pathBytes("song.mp3") }]))).rejects.toThrow("native path value leaked");
  });

  test("forwards the same ArrayBuffer without traversing byte values or attached fields", async () => {
    const bytes = new Uint8Array([7, 8, 9]).buffer;
    Object.defineProperty(bytes, "audit", { enumerable: true, get() { throw new Error("bytes walked"); } });
    const host = new FakeAttachmentHost([STAGED_RESPONSE]);
    await expect(stageChatAttachment({ ...STAGE_REQUEST, bytes }, host)).resolves.toMatchObject({ fileName: "large.ncm" });
    expect(host.calls[0]?.payload).toBe(bytes);
  });
});

function pathBytes(fileName: string): readonly string[] {
  return [`C:\\Users\\secret\\${fileName}`];
}

function swap(first: string, next: string): () => string {
  let calls = 0;
  return () => {
    calls += 1;
    return calls === 1 ? first : next;
  };
}
