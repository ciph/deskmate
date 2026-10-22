import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { Texture } from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { BUILTIN_PACKS } from "../src/pet/personaCatalog";
import { variantAnimationBounds } from "../src/pet/PetAnimation";
import { matchingVariantEnvelope, variantEnvelopeRecord, variantModelSha256, type VariantEnvelopeRecord } from "../src/pet/variantEnvelope";
import { disposeObject } from "../src/pet/PetRendererResources";

/** Reproducible source manifest, never supplied by an imported pack. Texture pixels do not affect skin geometry. */
export async function prepareBuiltinVariantEnvelopes(projectRoot: string): Promise<void> {
  const records: Record<string, VariantEnvelopeRecord> = {};
  for (const pack of BUILTIN_PACKS) {
    for (const persona of pack.personas) {
      if (persona.clipRoots === undefined) continue;
      const file = await readFile(resolve(projectRoot, "public/personas", persona.id, "figure.glb"));
      const bytes = file.buffer.slice(file.byteOffset, file.byteOffset + file.byteLength) as ArrayBuffer;
      const hash = await variantModelSha256(bytes);
      if (hash === null) throw new Error(`Envelope generation requires a self-contained GLB: ${persona.id}`);
      const loader = new GLTFLoader();
      loader.register(() => ({ name: "geometry_only_envelope", loadTexture: async () => new Texture() }));
      const gltf = await loader.parseAsync(bytes, "");
      try {
        gltf.scene.scale.setScalar(persona.scale);
        const bounds = variantAnimationBounds(gltf.scene, gltf.animations, persona.clipRoots);
        const record = variantEnvelopeRecord({ id: persona.id, scale: persona.scale, clipRoots: persona.clipRoots }, hash, bounds);
        if (matchingVariantEnvelope(record, record) === null) throw new Error(`Invalid animation envelope generated for ${persona.id}`);
        records[persona.id] = record;
      } finally { disposeObject(gltf.scene); }
    }
  }
  const output = resolve(projectRoot, "src/pet/variantEnvelopes.generated.json");
  const content = `${JSON.stringify(records, null, 2)}\n`;
  if (await readFile(output, "utf8").catch(() => "") !== content) await writeFile(output, content);
  console.log(`Prepared ${Object.keys(records).length} trusted variant envelopes`);
}
