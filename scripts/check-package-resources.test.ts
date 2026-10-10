import { expect, test } from "bun:test";
import { mkdtemp, mkdir, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { assertNoDebugFiles } from "./check-package-resources";

test("release resource check catches nested sourcemaps and PDBs", async () => {
  const root = await mkdtemp(join(tmpdir(), "yume-release-resources-"));
  try {
    await mkdir(join(root, "nested"));
    await writeFile(join(root, "app.js"), "export const value=1");
    await assertNoDebugFiles(root);
    for (const name of ["app.js.map", "taskkill.pdb"]) {
      const path = join(root, "nested", name);
      await writeFile(path, "debug");
      await expect(assertNoDebugFiles(root)).rejects.toThrow("Debug artifact");
      await rm(path);
    }
  } finally { await rm(root, { recursive: true, force: true }); }
});
