import { useCallback, useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Menu, MenuItem } from "@tauri-apps/api/menu";
import { getSettings, onPetScalePreview, onSettingsChanged, type Settings } from "../lib/settings";
import type { PetRenderer } from "./PetRenderer";
import { PetPomodoro } from "./PetPomodoro";
import { petLayout } from "./petLayout";
import { personaById } from "./personaCatalog";
import { GifPetView } from "./GifPetView";
import { usePackRevision } from "./usePackRevision";
import { usePetActivityState } from "./useGifState";
import { usePetSleep } from "./usePetSleep";
import type { SleepClock } from "./petSleep";
import { Rig2dPetView, rig2dEnvelope } from "./rig2d/Rig2dPetView";
import { useGifVisibility } from "./useGifVisibility";
import { usePetInteraction } from "./usePetInteraction";
import { useGifPassthrough } from "./useGifPassthrough";
import { gifEnvelope } from "./gifGeometry";
import { MorphPetView, type MorphPetHandle } from "./morph/MorphPetView";
import { morphEnvelope } from "./morph/desktopMotion";
import { FORM_KEYS } from "./morph/engine";
import { defaultGifTiming } from "./gifState";
import { PetAiUsageCache } from "./petAiUsage";
import { appendPetAiUsageMenu, type PetMenuItem } from "./petAiUsageMenu";
import type { LoadedGifPersona, loadGifPersona } from "./gifAssets";
import "./gifPet.css";

async function getSettingsWithRetry(): Promise<Settings> {
  let lastError: unknown = new Error("settings unavailable");
  for (let attempt = 0; attempt < 5; attempt += 1) {
    try { return await getSettings(); }
    catch (error: unknown) {
      lastError = error instanceof Error ? error : new Error(String(error));
      if (attempt < 4) await new Promise((resolve) => setTimeout(resolve, 100));
    }
  }
  throw lastError;
}
const report = (error: unknown) => console.error("pet interaction failed", error instanceof Error ? error.message : String(error));
const openSettings = () => { void invoke("open_settings").catch(report); };
const loadGlbView = () => import("./GlbPetView");

export default function PetApp({ gifLoad, glbLoad = loadGlbView, clock }: { readonly gifLoad?: typeof loadGifPersona; readonly glbLoad?: typeof loadGlbView; readonly clock?: SleepClock } = {}) {
  const rootRef = useRef<HTMLDivElement>(null);
  const rendererRef = useRef<PetRenderer | null>(null);
  const morphRef = useRef<MorphPetHandle | null>(null);
  const [usageCache] = useState(() => new PetAiUsageCache());
  const usageMenuRef = useRef<Awaited<ReturnType<typeof appendPetAiUsageMenu>> | null>(null);
  const contextMenuOpen = useRef(false);
  const latestSettings = useRef<Settings | null>(null);
  const mounted = useRef(false);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [petScale, setPetScale] = useState(0.5);
  const [readyGif, setReadyGif] = useState<{ readonly identity: { readonly personaId: string }; readonly data: LoadedGifPersona } | null>(null);
  const [readyRig, setReadyRig] = useState<object | null>(null);
  const [readyMorph, setReadyMorph] = useState<object | null>(null);
  const [readyGlb, setReadyGlb] = useState<object | null>(null);
  const [GlbView, setGlbView] = useState<typeof import("./GlbPetView").GlbPetView | null>(null);
  const [glbAttempt, setGlbAttempt] = useState(0);
  const personaId = settings?.personaId ?? "";
  const revision = usePackRevision(personaById(personaId).packId);
  const runtimeId = JSON.stringify([personaId, revision]);
  const personaIdentity = useMemo(() => ({ personaId, revision }), [personaId, revision]);
  const gif = settings !== null && personaById(personaId).renderType === "gif";
  const rig = settings !== null && personaById(personaId).renderType === "rig2d";
  const morph = settings !== null && personaById(personaId).renderType === "morph";
  const twoDimensional = gif || rig || morph;
  const gifData = readyGif?.identity === personaIdentity && loadError === null ? readyGif.data : null;
  const timing = useMemo(() => gifData === null ? defaultGifTiming : gifData.config.schemaVersion === 2 ? { ...gifData.config.feedback, thinkingSelection: gifData.config.thinkingSelection } : { ...gifData.config.feedback, thinkingEscalationMs: gifData.config.thinkingEscalationMs }, [gifData]);
  const { leaving, visible, nativeVisible } = useGifVisibility(personaId, twoDimensional, gifData?.config.leaving.durationMs ?? 910);
  const activity = usePetActivityState(visible, timing, runtimeId, undefined, clock);
  const ready = loadError === null && (morph ? readyMorph === personaIdentity : gif ? gifData !== null : rig ? readyRig === personaIdentity : readyGlb === personaIdentity);
  const sleep = usePetSleep({ visible: visible && !leaving, ready: ready && !morph, identity: runtimeId, idle: activity.idle, activityKey: activity.activityKey }, clock);
  const state = sleep.state === "sleep" && activity.idle ? "sleep" : activity.display;
  const layout = petLayout(petScale);
  const envelope = useMemo(() => morph ? morphEnvelope : rig ? rig2dEnvelope() : gifData?.config.schemaVersion === 2 ? gifEnvelope(gifData.config) : null, [gifData, rig, morph]);
  const setLocked = useGifPassthrough(twoDimensional && nativeVisible, rootRef);
  const interaction = usePetInteraction({ gif: twoDimensional, active: visible && !leaving && ready, identity: runtimeId, root: rootRef, onInteract: sleep.interact,
    onHeldChange: (held) => { sleep.setHeld(held); if (morph) morphRef.current?.hold(held); },
    ...(morph ? { onActivate: () => morphRef.current?.poke(), onDragChange: (held: boolean) => morphRef.current?.drag(held) } : {}),
  }, setLocked);
  const loaded = useCallback((data: LoadedGifPersona) => setReadyGif({ identity: personaIdentity, data }), [personaIdentity]);
  useEffect(() => {
    mounted.current = true;
    let disposed = false;
    const apply = (next: Settings) => {
      if (disposed) return;
      latestSettings.current = next;
      setSettings(next);
      setPetScale(next.petScale);
      usageMenuRef.current?.update(next);
    };
    let changed = false;
    const unlisten = onSettingsChanged((next) => { changed = true; apply(next); });
    void getSettingsWithRetry().then((next) => { if (!changed) apply(next); })
      .catch((error: unknown) => { if (!disposed) setLoadError(error instanceof Error ? error.message : String(error)); });
    const scale = onPetScalePreview((value) => { if (!disposed) setPetScale(value); });
    return () => { disposed = true; mounted.current = false; void usageMenuRef.current?.dispose(); void unlisten.then((stop) => stop()); void scale.then((stop) => stop()); };
  }, []);
  const loadedRig = useCallback(() => setReadyRig(personaIdentity), [personaIdentity]);
  const loadedMorph = useCallback(() => setReadyMorph(personaIdentity), [personaIdentity]);
  const loadedGlb = useCallback((loaded: boolean) => setReadyGlb(loaded ? personaIdentity : null), [personaIdentity]);
  const settingsReady = settings !== null;
  const glbLoading = settingsReady && !twoDimensional && !ready && loadError === null;
  useEffect(() => {
    if (!settingsReady || twoDimensional || GlbView !== null) return;
    let disposed = false;
    setLoadError(null);
    void glbLoad().then((module) => {
      if (!disposed) setGlbView(() => module.GlbPetView);
    }).catch((error: unknown) => {
      if (!disposed) setLoadError(error instanceof Error ? error.message : String(error));
    });
    return () => { disposed = true; };
  }, [settingsReady, twoDimensional, runtimeId, glbLoad, glbAttempt, GlbView]);
  const retryGlb = () => { setLoadError(null); setReadyGlb(null); setGlbAttempt((attempt) => attempt + 1); };
  useEffect(() => {
    if (settingsReady) void invoke("configure_pet_geometry", { gif: twoDimensional, envelope }).catch(report);
  }, [twoDimensional, envelope, settingsReady]);

  const onContextMenu = (event: MouseEvent) => {
    event.preventDefault();
    if (!settings || contextMenuOpen.current || !interaction.hit(event)) return;
    contextMenuOpen.current = true;
    const release = interaction.hold();
    void (async () => {
      const items: PetMenuItem[] = [];
      let menu: Menu | null = null;
      try {
        usageMenuRef.current = await appendPetAiUsageMenu(items, settings, usageCache, report);
        if (!mounted.current) return;
        if (latestSettings.current !== settings && latestSettings.current) usageMenuRef.current.update(latestSettings.current);
        if (morph) {
          const zh = settings?.language.startsWith("zh") ?? true;
          const status = morphRef.current?.snapshot();
          items.push(await MenuItem.new({ id: "morph-chat", text: zh ? "聊天（也可双击小著）" : "Chat (or double-click)", action: () => { void invoke("toggle_chat").catch(report); } }));
          const names = zh ? ["团子", "枕头", "年糕", "幽灵"] : ["Dumpling", "Pillow", "Rice cake", "Ghost"];
          const forms = await Promise.allSettled(FORM_KEYS.map((form, index) => MenuItem.new({ id: `morph-${form}`, text: `${status?.form === form ? "✓ " : ""}${names[index]}`, action: () => morphRef.current?.form(form) })));
          for (const result of forms) if (result.status === "fulfilled") items.push(result.value);
          const failed = forms.find(result => result.status === "rejected");
          if (failed?.status === "rejected") throw failed.reason;
          items.push(await MenuItem.new({ id: "morph-poke", text: zh ? "戳一下" : "Poke", enabled: activity.idle, action: () => morphRef.current?.poke() }));
          items.push(await MenuItem.new({ id: "morph-walk", text: zh ? "散个步" : "Take a walk", enabled: activity.idle, action: () => morphRef.current?.walk() }));
          items.push(await MenuItem.new({ id: "morph-sleep", text: status?.sleeping ? (zh ? "叫醒小著" : "Wake up") : (zh ? "睡个小觉" : "Take a nap"), enabled: activity.idle, action: () => morphRef.current?.sleep() }));
          items.push(await MenuItem.new({ id: "morph-auto", text: `${status?.auto ? "✓ " : ""}${zh ? "自己活动" : "Wander on its own"}`, action: () => morphRef.current?.autonomy() }));
        }
        items.push(await MenuItem.new({ id: "open-widget-settings", text: "小组件", action: () => { void invoke("open_widget_settings").catch(report); } }));
        items.push(await MenuItem.new({ id: "open-pet-settings", text: "设置", action: openSettings }));
        if (!twoDimensional) items.push(await MenuItem.new({ id: "poke-pet", text: "戳", action: () => rendererRef.current?.playNudge() }));
        menu = await Menu.new({ items });
        if (!mounted.current) return;
        await invoke("popup_pet_menu", { rid: menu.rid });
      } finally {
        await usageMenuRef.current?.dispose();
        usageMenuRef.current = null;
        const resources = menu === null ? items : [menu, ...items];
        const results = await Promise.allSettled(resources.map((resource) => resource.close()));
        for (const result of results) if (result.status === "rejected") report(result.reason);
      }
    })().catch(report).finally(() => { contextMenuOpen.current = false; release(); });
  };
  return <div ref={rootRef} className="pet-root" data-sleep-state={sleep.state} data-theme={settings?.theme ?? "dark"}>
    {settings !== null && <button type="button" className="pet-interaction" aria-label={morph ? "百变小著：单击互动，拖动拎起，双击聊天，右键切换形态" : "Open chat"} style={twoDimensional ? { width: layout.width * 2 * (envelope?.horizontal ?? 0.5), height: layout.width * ((envelope?.top ?? 1) + (envelope?.bottom ?? 0)) } : undefined}
      onMouseDown={interaction.onMouseDown} onMouseMove={interaction.onMouseMove} onMouseUp={interaction.onMouseUp} onContextMenu={onContextMenu}
      onDoubleClick={morph ? (event) => { if (interaction.hit(event)) void invoke("toggle_chat").catch(report); } : undefined}
      onKeyDown={(event) => {
        if (morph && visible && ready && !event.repeat) {
          const index = ["1", "2", "3", "4"].indexOf(event.key);
          if (index >= 0) { event.preventDefault(); morphRef.current?.form(FORM_KEYS[index]); return; }
          if (event.key === "Enter") { event.preventDefault(); void invoke("toggle_chat").catch(report); return; }
        }
        interaction.onKeyDown(event);
      }}>
      {morph ? <MorphPetView key={runtimeId} api={morphRef} width={layout.width} state={activity.display} visible={nativeVisible} leaving={leaving} onLoaded={loadedMorph} onError={setLoadError} />
        : gif ? <GifPetView key={runtimeId} personaId={personaId} revision={revision} state={leaving ? "leaving" : state} width={layout.width}
        leaving={leaving} visible={visible} onError={setLoadError} onLoaded={loaded} load={gifLoad} />
        : rig ? <Rig2dPetView key={runtimeId} personaId={personaId} revision={revision} state={state} width={layout.width}
          leaving={leaving} visible={nativeVisible} onError={setLoadError} onLoaded={loadedRig} />
        : GlbView !== null ? <GlbView key={`${runtimeId}:${glbAttempt}`} mood={activity.mood} visible={nativeVisible} settings={settings} width={layout.width} height={layout.height} rendererRef={rendererRef} onError={setLoadError} onReady={loadedGlb} /> : null}
    </button>}
    <PetPomodoro language={settings?.language ?? "zh-CN"} scale={petScale} />
    {glbLoading && <div role="status" className="pet-load-status">正在加载 3D 桌宠…</div>}
    {loadError !== null && <div role="alert" className="pet-load-error">桌宠加载失败 {settingsReady && !twoDimensional && <button type="button" onClick={retryGlb}>重试</button>} <button type="button" onClick={openSettings}>打开设置切换角色</button></div>}
  </div>;
}
