import { useEffect, useRef, useState } from "react";
import { createRoot } from "react-dom/client";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { RadialMenu } from "./RadialMenu";
import { UsageCloud } from "./UsageCloud";
import { MENU_UPDATE, type RadialModel } from "./model";

function PetMenuWindow() {
  const [model, setModel] = useState<RadialModel | null>(null);
  const [visibleId, setVisibleId] = useState<string | null>(null);
  const finished = useRef(false);
  useEffect(() => {
    let disposed = false;
    const apply = (next: RadialModel | null) => {
      if (!disposed && next) setModel(current => current?.id === next.id && current.revision > next.revision ? current : next);
    };
    const listener = listen<RadialModel>(MENU_UPDATE, event => apply(event.payload));
    void listener.then(() => invoke<RadialModel | null>("get_pet_radial_menu")).then(apply).catch(console.error);
    return () => { disposed = true; void listener.then(stop => stop()); };
  }, []);
  const id = model?.id;
  useEffect(() => {
    if (!id) return;
    let active = true;
    finished.current = false;
    void invoke("pet_radial_menu_ready", { id }).then(() => { if (active) setVisibleId(id); }).catch(console.error);
    return () => { active = false; };
  }, [id]);
  const finish = (action: string | null) => {
    if (!model) return;
    if (finished.current) return;
    finished.current = true;
    void invoke("finish_pet_radial_menu", { id: model.id, action }).catch(error => { finished.current = false; console.error(error); });
  };
  return model && (model.usage
    ? <UsageCloud key={model.id} model={model} usage={model.usage} visible={visibleId === model.id} onClose={() => finish(null)} />
    : <RadialMenu key={model.id} model={model} onFinish={finish} />);
}
createRoot(document.getElementById("root")!).render(<PetMenuWindow />);
