import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { inflateSync } from "node:zlib";
import golden from "../../docs/assets/xiaozhu-sandaime.golden.json";
import provenance from "../../docs/assets/xiaozhu-sandaime.provenance.json";

const sha = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
function canonical(value: unknown): unknown {
  if (Array.isArray(value)) return value.map(canonical);
  if (value && typeof value === "object") return Object.fromEntries(Object.entries(value).sort(([a], [b]) => a < b ? -1 : a > b ? 1 : 0).map(([key, item]) => [key, canonical(item)]));
  return value;
}
type View = { buffer: number; byteOffset?: number; byteLength?: number };
function paeth(a: number, b: number, c: number) {
  const p = a + b - c, da = Math.abs(p - a), db = Math.abs(p - b), dc = Math.abs(p - c);
  return da <= db && da <= dc ? a : db <= dc ? b : c;
}

// Independent PNG decoder: no Pillow or encoder code is used in this regression.
function decodePng(png: Buffer) {
  expect(png.subarray(0, 8).toString("hex")).toBe("89504e470d0a1a0a");
  const width = png.readUInt32BE(16), height = png.readUInt32BE(20);
  expect([...png.subarray(24, 29)]).toEqual([8, 6, 0, 0, 0]); // RGBA8, non-interlaced
  const idat: Buffer[] = [], metadata: Buffer[] = [];
  for (let pos = 8; pos < png.length;) {
    const end = pos + png.readUInt32BE(pos) + 12, chunk = png.subarray(pos, end);
    if (chunk.toString("ascii", 4, 8) === "IDAT") idat.push(chunk.subarray(8, -4));
    else metadata.push(chunk);
    pos = end;
  }
  const packed = inflateSync(Buffer.concat(idat)), stride = width * 4;
  expect(packed.length).toBe((stride + 1) * height);
  const pixels = Buffer.alloc(stride * height);
  for (let y = 0; y < height; y++) {
    const row = y * stride, src = y * (stride + 1), filter = packed[src]!;
    if (filter > 4) throw new Error("Invalid PNG filter");
    for (let x = 0; x < stride; x++) {
      const a = x >= 4 ? pixels[row + x - 4]! : 0, b = y ? pixels[row + x - stride]! : 0;
      const c = y && x >= 4 ? pixels[row + x - stride - 4]! : 0;
      const predictor = filter === 0 ? 0 : filter === 1 ? a : filter === 2 ? b : filter === 3 ? Math.floor((a + b) / 2) : paeth(a, b, c);
      pixels[row + x] = (packed[src + x + 1]! + predictor) & 255;
    }
  }
  return { width, height, rgbaSha256: sha(pixels), metadataSha256: sha(Buffer.concat(metadata)) };
}

test("lossless model preserves every original pixel, color chunk, scene, geometry and animation byte", async () => {
  const raw = Buffer.from(await Bun.file(new URL("../../public/personas/xiaozhu-sandaime/figure.glb", import.meta.url)).arrayBuffer());
  expect(raw.length).toBe(provenance.outputBytes); expect(raw.length).toBeLessThan(64 * 1024 * 1024);
  expect(sha(raw)).toBe(provenance.losslessOptimization.afterSha256);
  const jsonSize = raw.readUInt32LE(12);
  const doc = JSON.parse(raw.toString("utf8", 20, 20 + jsonSize)) as { buffers?: unknown; bufferViews: View[] };
  const binary = raw.subarray(28 + jsonSize), nonImages = createHash("sha256");
  let nonImageBytes = 0;
  for (const [index, view] of doc.bufferViews.entries()) {
    const data = binary.subarray(view.byteOffset ?? 0, (view.byteOffset ?? 0) + view.byteLength!);
    const original = golden.images.find(image => image.bufferView === index);
    if (original) { const { bufferView: _, ...expected } = original; expect(decodePng(data)).toEqual(expected); }
    else { nonImages.update(data); nonImageBytes += data.length; }
    delete view.byteOffset; delete view.byteLength;
  }
  delete doc.buffers;
  expect(sha(Buffer.from(JSON.stringify(canonical(doc))))).toBe(golden.sceneSha256);
  expect(nonImageBytes).toBe(golden.nonImageBytes); expect(nonImages.digest("hex")).toBe(golden.nonImageSha256);
}, 30_000);
