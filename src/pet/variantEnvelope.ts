import * as THREE from "three";
import { variantAnimationBounds } from "./PetAnimation";
import envelopes from "./variantEnvelopes.generated.json";

// Bump this when sampling, skin bounds, or scene preprocessing changes.
export const VARIANT_ENVELOPE_ALGORITHM = `variant-bounds-v1-three-${THREE.REVISION}-samples-24`;
const CACHE_LIMIT = 16;
type Roots = Readonly<Record<string, string>>;
export interface EnvelopeConfig { readonly id: string; readonly scale: number; readonly clipRoots: Roots; }
interface EnvelopeIdentity { readonly algorithm: string; readonly modelSha256: string; readonly scale: number; readonly clipRoots: Roots; }
export interface VariantEnvelopeRecord extends EnvelopeIdentity {
  readonly min: readonly number[];
  readonly max: readonly number[];
}
export interface ResolvedVariantEnvelope { readonly bounds: THREE.Box3; readonly source: "builtin" | "cache" | "computed"; }

function rootsKey(roots: Roots): string { return JSON.stringify(Object.entries(roots).sort(([a], [b]) => a.localeCompare(b))); }
function identityKey(identity: EnvelopeIdentity): string {
  return JSON.stringify([identity.algorithm, identity.modelSha256, identity.scale, rootsKey(identity.clipRoots)]);
}
function isRecord(value: unknown): value is Record<string, unknown> { return typeof value === "object" && value !== null && !Array.isArray(value); }
function validPoint(value: unknown): value is number[] { return Array.isArray(value) && value.length === 3 && value.every((part) => typeof part === "number" && Number.isFinite(part)); }

/** Only records compiled into the application or computed locally reach this reader. */
export function matchingVariantEnvelope(value: unknown, identity: EnvelopeIdentity): THREE.Box3 | null {
  if (!isRecord(value) || value.algorithm !== identity.algorithm || value.modelSha256 !== identity.modelSha256
    || value.scale !== identity.scale || !isRecord(value.clipRoots)
    || !Object.values(value.clipRoots).every((root) => typeof root === "string")
    || rootsKey(value.clipRoots as Roots) !== rootsKey(identity.clipRoots)
    || !validPoint(value.min) || !validPoint(value.max)) return null;
  const box = new THREE.Box3(new THREE.Vector3(...value.min), new THREE.Vector3(...value.max));
  return box.isEmpty() || box.getSize(new THREE.Vector3()).lengthSq() === 0 ? null : box;
}

/** A GLB referencing external geometry cannot be identified by hashing its main file. */
export function isSelfContainedGlb(bytes: ArrayBuffer): boolean {
  try {
    const view = new DataView(bytes);
    if (view.byteLength < 28 || view.getUint32(0, true) !== 0x46546c67 || view.getUint32(4, true) !== 2
      || view.getUint32(8, true) !== view.byteLength || view.getUint32(16, true) !== 0x4e4f534a) return false;
    const length = view.getUint32(12, true);
    if (length < 2 || length > view.byteLength - 20) return false;
    const json: unknown = JSON.parse(new TextDecoder().decode(new Uint8Array(bytes, 20, length)));
    if (!isRecord(json) || !Array.isArray(json.buffers) || json.buffers.length !== 1) return false;
    const buffer: unknown = json.buffers[0];
    const binaryOffset = 20 + length;
    return isRecord(buffer) && buffer.uri === undefined && typeof buffer.byteLength === "number"
      && Number.isInteger(buffer.byteLength) && buffer.byteLength > 0
      && binaryOffset + 8 <= bytes.byteLength && view.getUint32(binaryOffset + 4, true) === 0x004e4942
      && buffer.byteLength <= view.getUint32(binaryOffset, true)
      && binaryOffset + 8 + view.getUint32(binaryOffset, true) === bytes.byteLength;
  } catch { return false; }
}

/** Hash the exact buffer passed to GLTFLoader.parseAsync; no second fetch or URL-only identity. */
export async function variantModelSha256(bytes: ArrayBuffer): Promise<string | null> {
  if (!isSelfContainedGlb(bytes)) return null;
  try {
    const digest = await crypto.subtle.digest("SHA-256", bytes);
    return Array.from(new Uint8Array(digest), (byte) => byte.toString(16).padStart(2, "0")).join("");
  } catch { return null; } // Missing WebCrypto is a performance fallback, never a rendering failure.
}

export function variantEnvelopeRecord(config: EnvelopeConfig, modelSha256: string, bounds: THREE.Box3): VariantEnvelopeRecord {
  return { algorithm: VARIANT_ENVELOPE_ALGORITHM, modelSha256, scale: config.scale, clipRoots: { ...config.clipRoots }, min: bounds.min.toArray(), max: bounds.max.toArray() };
}

/** Session-local, bounded LRU. Pack metadata and browser storage never supply trusted envelopes. */
export class VariantEnvelopeCache {
  private readonly records = new Map<string, VariantEnvelopeRecord>();
  resolve(model: THREE.Object3D, clips: readonly THREE.AnimationClip[], config: EnvelopeConfig, modelSha256: string | null): ResolvedVariantEnvelope {
    model.parent?.updateWorldMatrix(true, false);
    const eligible = Number.isFinite(config.scale) && config.scale > 0
      && model.scale.equals(new THREE.Vector3(config.scale, config.scale, config.scale))
      && (model.parent === null || model.parent.matrixWorld.equals(new THREE.Matrix4()));
    const identity = eligible && modelSha256 !== null && /^[a-f0-9]{64}$/.test(modelSha256)
      ? { algorithm: VARIANT_ENVELOPE_ALGORITHM, modelSha256, scale: config.scale, clipRoots: config.clipRoots } : null;
    if (identity !== null) {
      const builtin = matchingVariantEnvelope((envelopes as Record<string, unknown>)[config.id], identity);
      if (builtin !== null) return { bounds: builtin, source: "builtin" };
      const key = identityKey(identity);
      const record = this.records.get(key);
      const cached = matchingVariantEnvelope(record, identity);
      if (cached !== null && record !== undefined) {
        this.records.delete(key);
        this.records.set(key, record);
        return { bounds: cached, source: "cache" };
      }
      this.records.delete(key);
    }
    const bounds = variantAnimationBounds(model, clips, config.clipRoots);
    if (identity !== null) {
      const record = variantEnvelopeRecord(config, identity.modelSha256, bounds);
      if (matchingVariantEnvelope(record, identity) !== null) {
        this.records.set(identityKey(identity), record);
        if (this.records.size > CACHE_LIMIT) this.records.delete(this.records.keys().next().value!);
      }
    }
    return { bounds, source: "computed" };
  }
}
export const variantEnvelopeCache = new VariantEnvelopeCache();
