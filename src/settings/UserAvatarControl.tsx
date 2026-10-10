import { useRef, useState, type ChangeEvent } from "react";
import { Avatar } from "../ui/Avatar";
import { prepareUserAvatar, userAvatarSource } from "../lib/userAvatar";
import type { Dict } from "../lib/i18n";
import "./user-avatar.css";

export function UserAvatarControl({ value, onSave, t }: {
  readonly value: string | null | undefined;
  readonly onSave: (value: string | null) => Promise<void>;
  readonly t: Dict;
}) {
  const input = useRef<HTMLInputElement>(null);
  const working = useRef(false);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const source = userAvatarSource(value);

  async function save(file: File | null) {
    if (working.current) return;
    working.current = true;
    setBusy(true);
    setError(null);
    try {
      let next: string | null = null;
      if (file) {
        try { next = await prepareUserAvatar(file); }
        catch (cause) {
          setError(cause instanceof Error && cause.message === "avatarTooLarge" ? t.avatarTooLarge : t.avatarInvalid);
          return;
        }
      }
      await onSave(next);
    } catch {
      setError(t.avatarSaveFailed);
    } finally {
      working.current = false;
      setBusy(false);
    }
  }

  function select(event: ChangeEvent<HTMLInputElement>) {
    const file = event.target.files?.[0];
    event.target.value = "";
    if (file) void save(file);
  }

  return <div className="set-avatar-control" aria-busy={busy}>
    <div className="set-avatar-actions">
      <Avatar sources={source ? [source] : []} label={t.myAvatar} />
      <input ref={input} type="file" accept="image/png,image/jpeg,image/webp" hidden aria-label={t.changeAvatar} onChange={select} />
      <button type="button" className="set-btn" disabled={busy} onClick={() => input.current?.click()}>{busy ? t.avatarSaving : t.changeAvatar}</button>
      <button type="button" className="set-btn" disabled={busy || !value} onClick={() => void save(null)}>{t.resetAvatar}</button>
    </div>
    <span className="set-avatar-hint">{t.avatarHint}</span>
    {error && <span className="set-avatar-error" role="alert">{error}</span>}
  </div>;
}
