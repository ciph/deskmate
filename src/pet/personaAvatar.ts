import { convertFileSrc } from "@tauri-apps/api/core";
import { appDataDir, join } from "@tauri-apps/api/path";
import { findPersonaById, packById } from "./personaCatalog";
import { isBuiltinPack, type AssetHost } from "./personaAssets";
import type { InstalledPack } from "../lib/packs";

export function builtinPersonaAvatar(id: string): readonly string[] {
  const persona = findPersonaById(id);
  if (!persona) return [];
  if (!isBuiltinPack(persona.packId)) return [];
  const cover = packById(persona.packId)?.thumbnail;
  return [`/persona-avatars/${persona.id}.png`, ...(cover ? [cover] : [])];
}

export async function personaAvatarSources(id: string, installed: readonly InstalledPack[],
  host: AssetHost = { appDataDir, join, convertFileSrc }): Promise<readonly string[]> {
  const persona = findPersonaById(id);
  if (!persona) return [];
  if (isBuiltinPack(persona.packId)) return builtinPersonaAvatar(id);
  const pack = installed.find(item => item.packId === persona.packId && item.personaIds.includes(persona.id));
  if (!pack) return [];
  const path = await host.join(await host.appDataDir(), "packs", persona.packId, "personas", persona.id, "avatar.png");
  return [host.convertFileSrc(path), ...(pack.thumbnailPath ? [host.convertFileSrc(pack.thumbnailPath)] : [])];
}
