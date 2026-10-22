import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { tmpdir } from "node:os";
import { prepareBuiltinVariantEnvelopes } from "./prepare-persona-envelopes";

test("missing bundled input fails explicit generation without clearing the previous manifest", async () => {
  const root = await mkdtemp(resolve(tmpdir(), "yume-envelope-input-"));
  try {
    const output = resolve(root, "src/pet/variantEnvelopes.generated.json");
    await mkdir(resolve(root, "src/pet"), { recursive: true });
    await writeFile(output, "previous trusted envelope\n");
    await expect(prepareBuiltinVariantEnvelopes(root)).rejects.toThrow();
    expect(await readFile(output, "utf8")).toBe("previous trusted envelope\n");
  } finally { await rm(root, { recursive: true, force: true }); }
});
