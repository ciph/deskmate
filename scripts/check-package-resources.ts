import { readdir } from "node:fs/promises";
import { resolve } from "node:path";

export async function assertNoDebugFiles(directory: string): Promise<void> {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const path = resolve(directory, entry.name);
    if (entry.isDirectory()) await assertNoDebugFiles(path);
    else if (/\.(map|pdb)$/i.test(entry.name)) throw new Error(`Debug artifact in release resources: ${path}`);
  }
}
if (import.meta.main) {
  const root = resolve(import.meta.dir, "..");
  await assertNoDebugFiles(resolve(root, "dist"));
  await assertNoDebugFiles(resolve(root, "src-tauri/resources/opencode"));
  const config = await Bun.file(resolve(root, "src-tauri/tauri.conf.json")).json();
  if (config.bundle.resources.some((path: string) => path.includes("windows-mcp"))) {
    throw new Error("Optional Windows component must not be bundled");
  }
  console.log("Release resources contain no debug files or bundled Windows MCP");
}
