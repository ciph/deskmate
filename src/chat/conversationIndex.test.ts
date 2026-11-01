import { expect, test } from "bun:test";
import { createConversationIndex } from "./conversationIndex";
test("streaming reuses membership and unaffected turns; insert/reorder/delete rebuild positions", () => {
  const index = createConversationIndex();
  const messages = [{id:"u",role:"user",text:"question"},{id:"a",role:"assistant",text:"answer"},{id:"u2",role:"user",text:"next"},{id:"a2",role:"assistant",text:"last"}];
  const first = index(messages), next = index([...messages.slice(0,3), {...messages[3]!,text:"last delta"}]);
  expect(next.ids).toBe(first.ids); expect(next.positions).toBe(first.positions); expect(next.turns[0]).toBe(first.turns[0]); expect(next.turns[1]!.summary).toBe("last delta");
  const reordered = index([messages[2]!,messages[3]!,messages[0]!]);
  expect([...reordered.ids]).toEqual(["u2","a2","u"]); expect(reordered.positions.get("u")).toBe(2); expect(reordered.lastUser).toBe("u");
  expect(index([]).turns).toEqual([]);
});
