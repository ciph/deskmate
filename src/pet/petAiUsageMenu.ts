import { invoke } from "@tauri-apps/api/core";
import { MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { dict } from "../lib/i18n";
import type { Settings } from "../lib/settings";
import { displayProviderLabel } from "../settings/aiProviderModel";
import { PetAiUsageCache, petAiUsageLines } from "./petAiUsage";

export type PetMenuItem = MenuItem | PredefinedMenuItem;

export async function appendPetAiUsageMenu(
  items: PetMenuItem[], settings: Settings, cache: PetAiUsageCache, report: (error: unknown) => void,
) {
  let disposed = false;
  let generation = 0;
  let writes = Promise.resolve();
  const snapshot = (next: Settings) => {
    const t = dict(next.language);
    const index = next.providers.findIndex(provider => provider.id === next.activeProviderId);
    const provider = next.providers[index] ?? null;
    const title = provider ? `${t.aiUsageTitle} · ${displayProviderLabel(provider, index, t)}` : t.aiUsageTitle;
    const result = cache.read(provider);
    const details = result.view.kind === "missing-key" ? t.petAiUsageConfigure : t.petAiUsageDetails;
    return { t, title, details, ...result };
  };
  const initial = snapshot(settings);
  const initialLines = petAiUsageLines(initial.view, initial.t);
  const rows: MenuItem[] = [];
  const created = await Promise.allSettled([initial.title, ...initialLines, initial.details].map((text, index) =>
    MenuItem.new({ id: `pet-ai-usage-${index}`, text: text.replaceAll("&", "&&"), enabled: index === 3,
      ...(index === 3 ? { action: () => { void invoke("open_ai_model_settings").catch(report); } } : {}) })));
  for (const result of created) if (result.status === "fulfilled") { items.push(result.value); rows.push(result.value); }
  const failed = created.find(result => result.status === "rejected");
  if (failed?.status === "rejected") throw failed.reason;
  items.push(await PredefinedMenuItem.new({ item: "Separator" }));

  const paint = (texts: readonly string[], version: number) => {
    writes = writes.then(async () => {
      if (disposed || version !== generation) return;
      // Native menu labels interpret ampersands as keyboard mnemonics.
      const results = await Promise.allSettled(rows.map((row, index) => row.setText(texts[index]!.replaceAll("&", "&&"))));
      for (const result of results) if (result.status === "rejected") report(result.reason);
    });
  };
  const show = (state: ReturnType<typeof snapshot>) => {
    const version = ++generation;
    const refreshing = state.refreshed !== null && state.view.kind === "ready";
    paint([refreshing ? `${state.title} · ${state.t.petAiUsageUpdating}` : state.title,
      ...petAiUsageLines(state.view, state.t), state.details], version);
    if (state.refreshed) void state.refreshed.then(view => {
      paint([state.title, ...petAiUsageLines(view, state.t), state.details], version);
    });
  };
  show(initial);
  await writes;
  return {
    update: (next: Settings) => { if (!disposed) show(snapshot(next)); },
    dispose: async () => { disposed = true; await writes; },
  };
}
