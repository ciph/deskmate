import { expect, test } from "bun:test";
import { mergeHistoryPage } from "./historyPages";
import type { CatalogLoadResult } from "../lib/unifiedHistory";

function page(from: number, to: number, cursor?: string): CatalogLoadResult {
  return { entry: { key: "native:a", identity: { kind: "native", sidecarId: "managed", directory: "/a", sessionId: "session" } } as CatalogLoadResult["entry"],
    nextCursor: cursor, messages: Array.from({ length: to - from }, (_, j) => ({ role: "assistant", text: `text ${from+j}`, time: 1,
      messageId: `msg_${String(from+j).padStart(4,"0")}`, partId: `part_${from+j}` })) };
}
test("equal-time cursor pages reconstruct every message once and preserve local replies", () => {
  const latest=page(100,200,"before100");
  latest.messages.push({ role: "user", text: "local", time: 2, messageId: "local_a", localOnly: true });
  const older=page(0,101);
  older.messages.push({ role: "user", text: "local", time: 2, messageId: "local_a", localOnly: true });
  const combined=mergeHistoryPage(latest,older,true);
  expect(combined.messages).toHaveLength(201);
  expect(new Set(combined.messages.map(m=>m.messageId)).size).toBe(201);
  expect(combined.messages[0]?.messageId).toBe("msg_0000");
  expect(combined.nextCursor).toBeUndefined();
});
test("live refresh preserves older pages, updates overlap and drops removed current-page rows", () => {
  const current=page(0,200,"older");
  const latest=page(100,201,"latest");
  latest.messages.splice(2,1);
  const next=mergeHistoryPage(current,latest,false);
  expect(next.messages).toHaveLength(200);
  expect(next.messages.some(m=>m.messageId==="msg_0102")).toBe(false);
  expect(next.nextCursor).toBe("older");
  expect(mergeHistoryPage(current,page(300,400,"gap"),false).nextCursor).toBe("gap");
});
test("foreign directory and opaque catalog key cannot be merged", () => {
  const source=page(0,100);
  const other: CatalogLoadResult={...source,entry:{...source.entry,identity:{kind:"native",sidecarId:"managed",directory:"/other",sessionId:"session"}}};
  expect(()=>mergeHistoryPage(page(100,200),other,true)).toThrow("history identity changed");
});

test("refresh replaces every part of the page boundary message, including removed first parts", () => {
  const current=page(0,200,"older");
  current.messages.push({...current.messages[100]!,partId:"aaa_removed",text:"removed"});
  const updated=mergeHistoryPage(current,page(100,201,"latest"),false);
  expect(updated.messages).toHaveLength(201);
  expect(updated.messages.some(message=>message.partId==="aaa_removed")).toBe(false);
});
