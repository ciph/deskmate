import type { Ref } from "react";
import glassesUrl from "./assets/milk-glasses.png";

interface Props {
  readonly id: string;
  readonly body: Ref<SVGPathElement>;
  readonly face: Ref<SVGGElement>;
  readonly mouth: Ref<SVGPathElement>;
  readonly feet: Ref<SVGGElement>;
}

/** Soft directional lighting and overlapping feet follow the shared motion engine. */
export function MilkArtwork({ id, body, face, mouth, feet }: Props) {
  return <>
    <defs>
      <radialGradient id={`${id}-milk`} cx=".3" cy=".18" r=".91">
        <stop stopColor="#fffef7" /><stop offset=".4" stopColor="#fff9f0" />
        <stop offset=".73" stopColor="#f5e6d9" /><stop offset="1" stopColor="#e8cebd" />
      </radialGradient>
      <radialGradient id={`${id}-light`} cx=".38" cy=".52" r=".6">
        <stop stopColor="#fffdf5" stopOpacity=".55" />
        <stop offset=".65" stopColor="#fffdf5" stopOpacity=".16" />
        <stop offset="1" stopColor="#fffdf5" stopOpacity="0" />
      </radialGradient>
      <radialGradient id={`${id}-cheek`} cx=".4" cy=".3" r=".7">
        <stop stopColor="#ffa297" /><stop offset=".7" stopColor="#ff8d80" />
        <stop offset=".94" stopColor="#ff8d80" stopOpacity=".88" />
        <stop offset="1" stopColor="#ff8d80" stopOpacity="0" />
      </radialGradient>
      <ellipse id={`${id}-cheek-shape`} cx="-54" cy="34" rx="14.5" ry="9" transform="rotate(-9 -54 34)" fill={`url(#${id}-cheek)`} />
      <filter id={`${id}-volume`} x="-.1" y="-.2" width="1.2" height="1.4" colorInterpolationFilters="sRGB">
        <feGaussianBlur in="SourceAlpha" stdDeviation="8" result="soft" />
        <feOffset in="soft" dx="-3" dy="-9" result="shade-offset" />
        <feComposite in="SourceAlpha" in2="shade-offset" operator="out" result="shade-mask" />
        <feFlood floodColor="#ae8368" floodOpacity=".27" />
        <feComposite in2="shade-mask" operator="in" result="shade" />
        <feOffset in="soft" dx="3" dy="5" result="light-offset" />
        <feComposite in="SourceAlpha" in2="light-offset" operator="out" result="light-mask" />
        <feFlood floodColor="#fffef8" floodOpacity=".3" />
        <feComposite in2="light-mask" operator="in" result="rim" />
        <feMerge><feMergeNode in="SourceGraphic" /><feMergeNode in="shade" /><feMergeNode in="rim" /></feMerge>
      </filter>
      <radialGradient id={`${id}-foot`} cx=".3" cy=".23" r=".84">
        <stop stopColor="#fffdf5" /><stop offset=".44" stopColor="#fbf1e4" />
        <stop offset=".79" stopColor="#ead6c3" /><stop offset="1" stopColor="#c4a28a" />
      </radialGradient>
      <radialGradient id={`${id}-occlusion`}>
        <stop stopColor="#78543e" stopOpacity=".32" />
        <stop offset=".48" stopColor="#99745b" stopOpacity=".17" />
        <stop offset="1" stopColor="#99745b" stopOpacity="0" />
      </radialGradient>
      <radialGradient id={`${id}-ground`}>
        <stop stopColor="#775641" stopOpacity=".9" />
        <stop offset=".55" stopColor="#94725b" stopOpacity=".48" />
        <stop offset="1" stopColor="#b49f8e" stopOpacity="0" />
      </radialGradient>
      <path ref={body} id={`${id}-outline`} data-morph-body="true" />
      <filter id={`${id}-glasses-shadow`} x="-.08" y="-.25" width="1.16" height="1.55" colorInterpolationFilters="sRGB">
        <feDropShadow dx=".5" dy="1.8" stdDeviation="1.2" floodColor="#62432d" floodOpacity=".3" />
      </filter>
    </defs>
    <g filter={`url(#${id}-volume)`}>
      <use href={`#${id}-outline`} fill={`url(#${id}-milk)`} />
      <use href={`#${id}-outline`} fill={`url(#${id}-light)`} />
    </g>
    <g ref={feet} data-milk-feet="true">
      {[0, 1].map(side => <g key={side}>
        <ellipse cx="3" cy="2" rx="22" ry="16" fill={`url(#${id}-occlusion)`} />
        <ellipse rx="16" ry="14" fill={`url(#${id}-foot)`} />
      </g>)}
    </g>
    <g ref={face} data-milk-face="true">
      <g data-morph-signature="true">
        <g data-milk-glasses="true" filter={`url(#${id}-glasses-shadow)`}>
          <svg x="-73" y="-22" width="146" height="44" viewBox="232 157 1709 447" preserveAspectRatio="none">
            <image href={glassesUrl} width="2172" height="724" />
          </svg>
        </g>
        <g data-milk-cheeks="true">
          <use href={`#${id}-cheek-shape`} />
          <use href={`#${id}-cheek-shape`} transform="scale(-1 1)" />
        </g>
      </g>
      <path ref={mouth} data-morph-mouth="true" fill="#302a28" stroke="#302a28" strokeLinejoin="round" strokeLinecap="round" />
    </g>
  </>;
}
