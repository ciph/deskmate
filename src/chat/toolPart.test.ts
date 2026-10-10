import { expect, test } from "bun:test";
import { toOpenCodeToolPart } from "./toolPart";
import { webSearchSites } from "./webSearchSites";

test("generic tool decoding preserves web search metadata after setup removal", () => {
  const part = toOpenCodeToolPart({
    id: "p", messageID: "m", sessionID: "s", callID: "c", type: "tool", tool: "websearch",
    state: { status: "running", title: "Search", input: { query: "test site:example.com https://example.com/docs" }, metadata: { urls: ["https://example.com"] } },
  });
  expect(part?.tool).toBe("websearch");
  expect(part?.state.metadata).toEqual({ urls: ["https://example.com"] });
  // The web-search consumer must still accept the decoded part without setup-specific logic.
  if (part) expect(webSearchSites(part)).toEqual(["example.com"]);
});

test("malformed tool events are rejected without reading or rendering their output", () => {
  expect(toOpenCodeToolPart(null)).toBeNull();
  expect(toOpenCodeToolPart({ type: "tool", state: { status: "completed", output: "private" } })).toBeNull();
  expect(toOpenCodeToolPart({ id: "p", messageID: "m", sessionID: "s", callID: "c", type: "tool", tool: "custom", state: { status: "unknown" } })).toBeNull();
});
