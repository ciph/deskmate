import { useState } from "react";
import { UserIcon } from "@phosphor-icons/react/dist/csr/User";
import "./avatar.css";

interface AvatarProps {
  readonly sources: readonly string[];
  readonly label: string;
  readonly fallback?: string;
}

export function Avatar(props: AvatarProps) {
  return <AvatarImage key={props.sources.join("\n")} {...props} />;
}

function AvatarImage({ sources, label, fallback }: AvatarProps) {
  const [index, setIndex] = useState(0);
  const source = sources[index];
  return (
    <span className="yume-avatar" role="img" aria-label={label}>
      {source ? <img key={source} src={source} alt="" width={32} height={32} draggable={false}
        onError={() => setIndex(current => current + 1)} />
        : fallback ? <span aria-hidden="true">{Array.from(fallback).slice(0, 2).join("")}</span>
        : <UserIcon size={20} aria-hidden="true" />}
    </span>
  );
}
