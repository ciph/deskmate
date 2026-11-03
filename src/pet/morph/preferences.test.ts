import { afterEach, expect, test } from "bun:test";
import { loadMorphPreferences, saveMorphPreferences } from "./preferences";

afterEach(() => localStorage.clear());

test("old preferences gain milk appearance without losing form or autonomy", () => {
  localStorage.setItem("yume-xiaozhu-morph-v1", JSON.stringify({ form: "ghost", auto: false }));
  expect(loadMorphPreferences()).toEqual({ form: "ghost", auto: false, appearance: "milk" });
  saveMorphPreferences({ form: "pillow", auto: false, appearance: "classic" });
  expect(loadMorphPreferences()).toEqual({ form: "pillow", auto: false, appearance: "classic" });
});

test("invalid preference fields fall back safely", () => {
  localStorage.setItem("yume-xiaozhu-morph-v1", JSON.stringify({ form: "unknown", appearance: "unknown" }));
  expect(loadMorphPreferences()).toEqual({ form: "mochi", auto: true, appearance: "milk" });
});
