import type { InvokeOptions } from "@tauri-apps/api/core";

export type BinaryUploadIdentity = { readonly uploadId: string; readonly scopeKey: string };
export type BinaryUploadMetadata = BinaryUploadIdentity & {
  readonly sessionId?: string;
  readonly fileName: string;
  readonly mime: string;
  readonly size: number;
};

export function binaryUploadOptions(metadata: BinaryUploadMetadata): InvokeOptions {
  const encoder = new TextEncoder();
  if (!metadata.uploadId || metadata.uploadId.length > 128 || /[^A-Za-z0-9._:-]/.test(metadata.uploadId)
    || !metadata.scopeKey.trim() || encoder.encode(metadata.scopeKey).byteLength > 1024 || /[\u0000-\u001f\u007f-\u009f]/.test(metadata.scopeKey)) {
    throw new Error("invalid upload identity");
  }
  const bytes = encoder.encode(JSON.stringify(metadata));
  if (bytes.length > 6144) throw new Error("upload metadata too large");
  return { headers: { "x-yume-upload": btoa(String.fromCharCode(...bytes)) } };
}

export function binaryUploadValue(response: unknown, identity: BinaryUploadIdentity): unknown {
  if (!response || typeof response !== "object" || !("uploadId" in response) || !("scopeKey" in response)
    || response.uploadId !== identity.uploadId || response.scopeKey !== identity.scopeKey || !("value" in response)) {
    throw new Error("upload response identity mismatch");
  }
  return response.value;
}
