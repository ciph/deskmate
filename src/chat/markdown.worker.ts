import { parseMarkdown } from "./markdownPipeline";
let owner = 0;
let source = "";
self.onmessage = (event: MessageEvent<{ id: number; version: number; reset?: string; append?: string; streaming: boolean }>) => {
  const request = event.data;
  if (request.reset !== undefined) { owner = request.id; source = request.reset; }
  else if (owner !== request.id) throw new Error("Markdown source scope mismatch");
  else source += request.append ?? "";
  self.postMessage({ id: request.id, version: request.version, ...parseMarkdown(source, request.streaming) });
};
