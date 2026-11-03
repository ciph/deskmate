import { isFormKey, type FormKey } from "./engine";
export type MorphAppearance = "milk" | "classic";
export interface MorphPreferences {
  form: FormKey;
  auto: boolean;
  appearance: MorphAppearance;
}
const key = "yume-xiaozhu-morph-v1";
export function loadMorphPreferences(): MorphPreferences {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? "{}");
    if (value && typeof value === "object") {
      const form =
        "form" in value && isFormKey(value.form) ? value.form : "mochi";
      const appearance = "appearance" in value && value.appearance === "classic" ? "classic" : "milk";
      return { form, auto: !("auto" in value) || value.auto !== false, appearance };
    }
  } catch (error) {
    console.warn("Xiaozhu preferences unavailable", error);
  }
  return { form: "mochi", auto: true, appearance: "milk" };
}
export function saveMorphPreferences(value: MorphPreferences): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (error) {
    console.warn("Xiaozhu preferences could not be saved", error);
  }
}
