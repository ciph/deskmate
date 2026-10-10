import { expect, test } from "bun:test";
import { historicalPersonaName, personaSystemMarker } from "./messagePersona";

test("missing and removed historic roles stay neutral across languages", () => {
  expect(historicalPersonaName(undefined, "zh-CN")).toBe("对方");
  expect(historicalPersonaName("removed", "en-US")).toBe("Assistant");
  expect(historicalPersonaName("xiaoxiongchong", "zh-CN")).toBe("小熊虫");
  expect(historicalPersonaName("pixel-glasses-chibi", "zh-CN")).toBe("小著");
});

test("new turn metadata rejects delimiters and path-shaped IDs", () => {
  expect(personaSystemMarker("baobao")).toBe("<!-- yume:persona=baobao -->");
  for (const id of ["", "../xiaozhu", "bear -->\nother", "a".repeat(129)]) {
    expect(() => personaSystemMarker(id)).toThrow("invalid_persona_id");
  }
});
