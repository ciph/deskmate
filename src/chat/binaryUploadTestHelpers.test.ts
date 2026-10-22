import type { InvokeOptions } from "@tauri-apps/api/core";

type UploadMetadata = {
  readonly uploadId: string;
  readonly scopeKey: string;
  readonly sessionId?: string;
  readonly fileName: string;
  readonly mime: string;
  readonly size: number;
};

export function decodeUploadOptions(options?: InvokeOptions): UploadMetadata {
  const header = new Headers(options?.headers).get("x-yume-upload");
  if (!header) throw new Error("binary upload metadata header missing");
  const decoded: unknown = JSON.parse(new TextDecoder("utf-8", { fatal: true }).decode(Uint8Array.from(atob(header), char => char.charCodeAt(0))));
  if (typeof decoded !== "object" || decoded === null || Array.isArray(decoded)) throw new Error("binary upload metadata missing");
  const metadata = decoded as Record<string, unknown>;
  for (const field of ["uploadId", "scopeKey", "fileName", "mime"]) {
    if (typeof metadata[field] !== "string") throw new Error(`binary upload ${field} missing`);
  }
  if (typeof metadata.size !== "number" || !Number.isSafeInteger(metadata.size) || metadata.size < 0) throw new Error("binary upload size missing");
  if (metadata.sessionId !== undefined && typeof metadata.sessionId !== "string") throw new Error("binary upload sessionId invalid");
  return metadata as UploadMetadata;
}

export function requireBinaryUpload(body: unknown, options?: InvokeOptions): UploadMetadata & { readonly bytes: ArrayBuffer } {
  if (!(body instanceof ArrayBuffer)) throw new Error("raw ArrayBuffer upload body expected");
  const metadata = decodeUploadOptions(options);
  if (body.byteLength !== metadata.size) throw new Error("binary upload body length differs from metadata");
  return { ...metadata, bytes: body };
}

export function wrapUploadResponse<T>(options: InvokeOptions | undefined, value: T): { uploadId: string; scopeKey: string; value: T } {
  const { uploadId, scopeKey } = decodeUploadOptions(options);
  return { uploadId, scopeKey, value };
}
