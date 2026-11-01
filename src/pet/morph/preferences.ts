import { isFormKey, type FormKey } from "./engine";
export interface MorphPreferences {
  form: FormKey;
  auto: boolean;
}
const key = "yume-xiaozhu-morph-v1";
export function loadMorphPreferences(): MorphPreferences {
  try {
    const value: unknown = JSON.parse(localStorage.getItem(key) ?? "{}");
    if (value && typeof value === "object") {
      const form =
        "form" in value && isFormKey(value.form) ? value.form : "mochi";
      return { form, auto: !("auto" in value) || value.auto !== false };
    }
  } catch (error) {
    console.warn("Xiaozhu preferences unavailable", error);
  }
  return { form: "mochi", auto: true };
}
export function saveMorphPreferences(value: MorphPreferences): void {
  try {
    localStorage.setItem(key, JSON.stringify(value));
  } catch (error) {
    console.warn("Xiaozhu preferences could not be saved", error);
  }
}
