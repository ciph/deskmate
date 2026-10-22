import { useEffect, useMemo, useState } from "react";
import { listInstalledPacks, onPackImported } from "../lib/packs";
import { builtinPersonaAvatar, personaAvatarSources } from "../pet/personaAvatar";

/** Resolve once per distinct role, including mixed-persona transcripts. */
export function usePersonaAvatars(personaIds: readonly string[]): Readonly<Record<string, readonly string[]>> {
  const key = JSON.stringify([...new Set(personaIds)].sort());
  const [loaded, setLoaded] = useState<{ key: string; sources: Record<string, readonly string[]> } | null>(null);
  useEffect(() => {
    const ids: string[] = JSON.parse(key);
    const imported = ids.filter(id => !builtinPersonaAvatar(id).length);
    if (!imported.length) return;
    let disposed = false;
    let revision = 0;
    async function refresh(version?: string) {
      const request = ++revision;
      try {
        const packs = await listInstalledPacks();
        const entries = await Promise.all(imported.map(async id => {
          const sources = await personaAvatarSources(id, packs);
          return [id, version ? sources.map(source => `${source}?avatarVersion=${version}`) : sources] as const;
        }));
        if (!disposed && request === revision) setLoaded({ key, sources: Object.fromEntries(entries) });
      } catch {
        if (!disposed && request === revision) setLoaded({ key, sources: {} });
      }
    }
    const stop = onPackImported(pack => { void refresh(pack.sha256); }).catch(() => () => {});
    void refresh();
    return () => { disposed = true; void stop.then(unlisten => unlisten()).catch(() => {}); };
  }, [key]);
  return useMemo(() => Object.fromEntries((JSON.parse(key) as string[]).map(id => {
    const builtin = builtinPersonaAvatar(id);
    return [id, builtin.length ? builtin : loaded?.key === key ? loaded.sources[id] ?? [] : []];
  })), [key, loaded]);
}
