import { expect, test } from "bun:test";
import { realpathSync, mkdtempSync, writeFileSync, unlinkSync, rmdirSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { tmpdir } from "node:os";
import { fileURLToPath } from "node:url";
import { build, resolveConfig } from "vite";

test("uses the physical project root so junction builds emit local HTML paths", async () => {
  // Given: config loading may start from a junction or another working directory.
  const configFile = fileURLToPath(new URL("./vite.config.ts", import.meta.url));
  const projectRoot = realpathSync(dirname(configFile)).replaceAll("\\", "/");

  // When: Vite resolves the production configuration.
  const config = await resolveConfig({ configFile }, "build");

  // Then: its root shares the same physical identity as Rollup's HTML modules.
  expect(config.root).toBe(projectRoot);
});

test("the production Markdown bundle runs without a browser document", async () => {
  const configFile = fileURLToPath(new URL("./vite.config.ts", import.meta.url));
  const config = await resolveConfig({ configFile }, "build");
  const result = await build({ configFile: false, resolve: config.resolve, logLevel: "silent", build: {
    write: false, lib: { entry: resolve(dirname(configFile), "src/chat/markdown.worker.ts"), formats: ["es"] },
  } });
  if ("close" in result || Array.isArray(result) && result.length !== 1) throw new Error("Unexpected build result");
  const chunk = (Array.isArray(result) ? result[0]! : result).output.find(item => item.type === "chunk");
  if (!chunk || chunk.type !== "chunk") throw new Error("Missing worker code");
  const directory = mkdtempSync(join(tmpdir(), "yume-markdown-worker-"));
  const file = join(directory, "worker.mjs"); writeFileSync(file, chunk.code);
  const worker = new Worker(file);
  try {
    const result = await new Promise<{ blocks: unknown[] }>((resolve, reject) => {
      const timer = setTimeout(() => reject(new Error("Worker timed out")), 5000);
      worker.onmessage = event => { clearTimeout(timer); resolve(event.data); };
      worker.onerror = event => { clearTimeout(timer); reject(new Error(event.message)); };
      worker.postMessage({ id: 1, version: 1, reset: "**bold** &copy; &notit;", streaming: false });
    });
    expect(JSON.stringify(result.blocks)).toContain("strong");
    expect(JSON.stringify(result.blocks)).toContain("©");
  } finally { worker.terminate(); unlinkSync(file); rmdirSync(directory); }
});
