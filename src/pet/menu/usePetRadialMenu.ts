import { useEffect, useRef } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
import { getAiUsage, type Settings } from "../../lib/settings";
import { dict } from "../../lib/i18n";
import { usageCloud, usageError } from "./usageCloudData";
import type { MorphPetHandle } from "../morph/MorphPetView";
import { FORM_KEYS } from "../morph/engine";
import type { GifRect } from "../gifGeometry";
import { personaById } from "../personaCatalog";
import { findAction, MENU_RESULT, type MenuResult, type RadialItem, type RadialModel } from "./model";

const report = (error: unknown) => console.error("pet radial menu failed", error);
type Options = {
  readonly settings: Settings | null; readonly identity: string; readonly active: boolean; readonly idle: boolean;
  readonly anchor: () => GifRect | null;
  readonly morph: MorphPetHandle | null; readonly nudge?: () => void; readonly hold: (persistOnBlur?: boolean) => () => void;
};
interface Session {
  model: RadialModel; actions: Map<string, () => void>; release: () => void; opened: boolean;
}

export function usePetRadialMenu(options: Options) {
  const latest = useRef(options); latest.current = options;
  const active = useRef<Session | null>(null);
  const listener = useRef<Promise<UnlistenFn> | null>(null);
  const cancel = () => {
    const session = active.current;
    if (!session) return;
    active.current = null; session.release();
    void invoke("finish_pet_radial_menu", { id: session.model.id, action: null }).catch(report);
  };
  useEffect(() => {
    const pending = listen<MenuResult>(MENU_RESULT, ({ payload }) => {
      const session = active.current;
      if (!session || session.model.id !== payload.id) return;
      active.current = null; session.release();
      if (payload.action && findAction(session.model.items, payload.action)) session.actions.get(payload.action)?.();
    });
    listener.current = pending;
    void pending.catch(report);
    return () => { cancel(); listener.current = null; void pending.then(stop => stop()).catch(report); };
  }, []);
  const publish = (session: Session) => {
    if (active.current === session && session.opened) void invoke("update_pet_radial_menu", { menu: session.model }).catch(report);
  };
  const refresh = (session: Session) => {
    if (session.model.usage) return;
    const { settings, idle, morph, nudge } = latest.current;
    if (!settings) return;
    const zh = settings.language.startsWith("zh");
    const actions = new Map<string, () => void>();
    const command = (name: string) => () => { void invoke(name).catch(report); };
    const item = (id: string, label: string, icon: RadialItem["icon"], action: () => void, enabled = true, checked?: boolean): RadialItem => {
      actions.set(id, action); return { id, label, icon, enabled, checked };
    };
    const group = (id: string, label: string, icon: RadialItem["icon"], children: readonly RadialItem[]): RadialItem => ({ id, label, icon, enabled: true, children });
    const items: RadialItem[] = [item("chat", zh ? "聊天" : "Chat", "chat", command("toggle_chat"))];
    if (morph) {
      const status = morph.snapshot();
      items.push(group("actions", zh ? "动作" : "Actions", "play", [
        item("morph-poke", zh ? "戳一下" : "Poke", "poke", () => morph.poke(), idle),
        item("morph-walk", zh ? "散步" : "Walk", "walk", () => morph.walk(), idle),
        item("morph-sleep", status.sleeping ? (zh ? "叫醒" : "Wake up") : (zh ? "睡觉" : "Sleep"), "sleep", () => morph.sleep(), idle, status.sleeping),
        item("morph-auto", zh ? "自己活动" : "Wander", "play", () => morph.autonomy(), true, status.auto),
      ]));
      const names = zh ? ["团子", "枕头", "年糕", "幽灵"] : ["Dumpling", "Pillow", "Rice cake", "Ghost"];
      items.push(group("forms", zh ? "形态" : "Forms", "pet", FORM_KEYS.map((form, i) => item(`morph-${form}`, names[i], "pet", () => morph.form(form), true, status.form === form))));
      items.push(group("appearance", zh ? "外观" : "Appearance", "appearance", [
        item("morph-appearance-milk", zh ? "奶团质感" : "Soft milk", "appearance", () => morph.appearance("milk"), true, status.appearance === "milk"),
        item("morph-appearance-classic", zh ? "经典 2D" : "Classic 2D", "pet", () => morph.appearance("classic"), true, status.appearance === "classic"),
      ]));
    } else if (nudge) items.push(item("poke-pet", zh ? "戳一下" : "Poke", "poke", nudge, idle));
    items.push(item("ai-usage", zh ? "AI 用量" : "AI usage", "ai", () => openUsage()));
    items.push(group("more", zh ? "更多" : "More", "widget", [
      item("open-widget-settings", zh ? "小组件" : "Widgets", "widget", command("open_widget_settings")),
      item("open-pet-settings", zh ? "设置" : "Settings", "general", command("open_settings")),
    ]));
    session.actions = actions;
    session.model = { ...session.model, revision: session.model.revision + 1, theme: settings.theme, language: settings.language,
      title: personaById(settings.personaId).name[zh ? "zh" : "en"], items };
    publish(session);
  };
  useEffect(() => { if (active.current) refresh(active.current); }, [options.settings, options.idle]);
  useEffect(() => { if (active.current?.model.usage) cancel(); }, [options.settings]);
  useEffect(() => { if (!options.active) cancel(); return cancel; }, [options.active, options.identity]);
  const open = (session: Session) => {
    void (async () => {
      await listener.current;
      if (active.current !== session) return;
      await invoke("open_pet_radial_menu", { menu: session.model });
      if (active.current !== session) { await invoke("finish_pet_radial_menu", { id: session.model.id, action: null }); return; }
      session.opened = true;
      publish(session);
    })().catch(error => { if (active.current === session) cancel(); report(error); });
  };
  const createSession = (): Session => {
    const session: Session = { model: { id: crypto.randomUUID(), revision: 0, title: "", theme: "dark", language: "zh-CN", anchor: latest.current.anchor(), items: [] }, actions: new Map(), release: latest.current.hold(true), opened: false };
    active.current = session;
    return session;
  };
  const openUsage = () => {
    const { settings, active: enabled } = latest.current;
    if (!enabled || !settings || active.current || !listener.current) return;
    const t = dict(settings.language);
    const provider = settings.providers.find(provider => provider.id === settings.activeProviderId);
    const session = createSession();
    session.model = { ...session.model, title: t.aiUsageTitle, theme: settings.theme, language: settings.language,
      usage: { label: "", percent: null, amount: "—", note: provider?.apiKey.trim() ? t.aiUsageLoading : t.aiUsageMissingKey } };
    open(session);
    if (!provider?.apiKey.trim()) return;
    void getAiUsage(provider.id).then(usage => {
      if (active.current !== session) return;
      session.model = { ...session.model, revision: session.model.revision + 1, usage: usageCloud(usage, t) };
      publish(session);
    }).catch(error => {
      if (active.current !== session) return;
      session.model = { ...session.model, revision: session.model.revision + 1,
        usage: { label: "", percent: null, amount: "—", note: usageError(error, t) } };
      publish(session);
    });
  };
  return () => {
    if (!latest.current.active || !latest.current.settings || active.current || !listener.current) return;
    const session = createSession();
    refresh(session);
    open(session);
  };
}
