import { describe, expect, it, spyOn } from "bun:test";
import * as THREE from "three";
import { matchingVariantEnvelope, isSelfContainedGlb, variantModelSha256, VariantEnvelopeCache, variantEnvelopeRecord, VARIANT_ENVELOPE_ALGORITHM } from "./variantEnvelope";

const hash = "a".repeat(64);
const config = { id: "imported-fixture", scale: 1, clipRoots: { move: "variant" } };
function fixture() {
  const model = new THREE.Group();
  const variant = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshBasicMaterial());
  variant.name = "variant";
  model.add(variant);
  const clips = [new THREE.AnimationClip("move", 1, [new THREE.NumberKeyframeTrack("variant.position[x]", [0, 1], [0, 10])])];
  return { model, variant, clips };
}
function glb(external = false): ArrayBuffer {
  const text = JSON.stringify({ asset: { version: "2.0" }, buffers: [{ byteLength: 4, ...(external ? { uri: "geometry.bin" } : {}) }] });
  const json = new TextEncoder().encode(text.padEnd(Math.ceil(text.length / 4) * 4, " "));
  const buffer = new ArrayBuffer(28 + json.length + 4);
  const view = new DataView(buffer);
  [0x46546c67, 2, buffer.byteLength, json.length, 0x4e4f534a].forEach((value, index) => view.setUint32(index * 4, value, true));
  new Uint8Array(buffer, 20, json.length).set(json);
  view.setUint32(20 + json.length, 4, true);
  view.setUint32(24 + json.length, 0x004e4942, true);
  return buffer;
}

describe("variant envelope identity", () => {
  it("rejects missing, old, malformed, and mismatched trusted records", () => {
    const bounds = new THREE.Box3(new THREE.Vector3(-1, 0, -1), new THREE.Vector3(1, 2, 1));
    const record = variantEnvelopeRecord(config, hash, bounds);
    expect(matchingVariantEnvelope(record, record)?.equals(bounds)).toBe(true);
    for (const value of [null, {}, { ...record, algorithm: "old" }, { ...record, modelSha256: "b".repeat(64) },
      { ...record, scale: 2 }, { ...record, clipRoots: { move: "other" } }, { ...record, min: [0, NaN, 0] },
      { ...record, max: [Infinity, 2, 1] }, { ...record, max: [-2, -2, -2] }, { ...record, min: [0, 0] },
      { ...record, min: [0, 0, 0], max: [0, 0, 0] }]) {
      expect(matchingVariantEnvelope(value, record)).toBeNull();
    }
    expect(record.algorithm).toBe(VARIANT_ENVELOPE_ALGORITHM);
  });
  it("binds the digest to self-contained bytes, refusing external or truncated buffers", async () => {
    const bytes = glb();
    expect(isSelfContainedGlb(bytes)).toBe(true);
    const first = await variantModelSha256(bytes);
    expect(first).toMatch(/^[a-f0-9]{64}$/);
    new Uint8Array(bytes)[bytes.byteLength - 1] = 1;
    expect(await variantModelSha256(bytes)).not.toBe(first);
    expect(await variantModelSha256(glb(true))).toBeNull();
    expect(await variantModelSha256(bytes.slice(0, -1))).toBeNull();
    expect(await variantModelSha256(new ArrayBuffer(0))).toBeNull();
  });
  it("falls back when WebCrypto is unavailable", async () => {
    const digest = spyOn(crypto.subtle, "digest").mockRejectedValue(new Error("unavailable"));
    try { expect(await variantModelSha256(glb())).toBeNull(); } finally { digest.mockRestore(); }
  });
});

describe("bounded session envelope cache", () => {
  it("computes unknown imports once, keeps independent bounds, and rejects identity changes", () => {
    const cache = new VariantEnvelopeCache();
    const { model, clips, variant } = fixture();
    const first = cache.resolve(model, clips, config, hash);
    expect(first.source).toBe("computed");
    expect(first.bounds.max.x).toBeCloseTo(10.5);
    expect(variant.position.x).toBe(0);
    first.bounds.max.x = 999;
    const second = cache.resolve(model, clips, config, hash);
    expect(second.source).toBe("cache");
    expect(second.bounds.max.x).toBeCloseTo(10.5);
    second.bounds.min.x = -999;
    expect(cache.resolve(model, clips, config, hash).bounds.min.x).toBeCloseTo(-0.5);
    expect(cache.resolve(model, clips, config, "b".repeat(64)).source).toBe("computed");
    model.scale.setScalar(2);
    expect(cache.resolve(model, clips, { ...config, scale: 2 }, hash).bounds.max.x).toBeCloseTo(21);
    model.scale.setScalar(1);
    const another = variant.clone(); another.name = "other"; model.add(another);
    expect(cache.resolve(model, clips, { ...config, clipRoots: { move: "other" } }, hash).source).toBe("computed");
  });
  it("does not trust pack envelope claims or cache unidentified/preprocessed geometry", () => {
    const cache = new VariantEnvelopeCache();
    const { model, clips } = fixture();
    const claimed = { ...config, envelope: { min: [-999, -999, -999], max: [999, 999, 999] } };
    expect(cache.resolve(model, clips, claimed, null).bounds.max.x).toBeCloseTo(10.5);
    expect(cache.resolve(model, clips, claimed, null).source).toBe("computed");
    const parent = new THREE.Group(); parent.position.x = 100; parent.add(model);
    expect(cache.resolve(model, clips, config, hash).source).toBe("computed");
    expect(cache.resolve(model, clips, config, hash).bounds.max.x).toBeCloseTo(110.5);
  });
  it("evicts least recently used entries after sixteen distinct model identities", () => {
    const cache = new VariantEnvelopeCache();
    const { model, clips } = fixture();
    const hashes = Array.from({ length: 17 }, (_, index) => index.toString(16).padStart(64, "0"));
    for (const value of hashes.slice(0, 16)) expect(cache.resolve(model, clips, config, value).source).toBe("computed");
    expect(cache.resolve(model, clips, config, hashes[0]).source).toBe("cache");
    cache.resolve(model, clips, config, hashes[16]);
    expect(cache.resolve(model, clips, config, hashes[0]).source).toBe("cache");
    expect(cache.resolve(model, clips, config, hashes[1]).source).toBe("computed");
  });
});
