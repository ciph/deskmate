import { virtualLayout } from "../testing/virtualLayout";
import { beforeEach, afterEach, describe, expect, test } from "bun:test";
import { cleanup, fireEvent, render } from "@testing-library/react";
import { createRef } from "react";
import { ChatNavigation } from "./ChatNavigation";
import { createConversationIndex, conversationTurns, activeTurnIndex } from "./conversationIndex";
import type { TimelineControl } from "./ChatTimeline";
let restoreLayout = () => {};
beforeEach(() => { restoreLayout = virtualLayout(); });
afterEach(() => { cleanup(); restoreLayout(); });
describe("conversation navigation", () => {
  test("groups replies under user turns and excludes artifact/tool-only rows", () => {
    expect(conversationTurns([
      { id: "intro", role: "assistant", text: "Welcome" },
      { id: "u1", role: "user", text: "First question" },
      { id: "a1", role: "assistant", text: "**First** answer" },
      { id: "file", role: "artifact" },
      { id: "a2", role: "assistant", text: "More detail" },
      { id: "u2", role: "user", text: "" },
    ])).toEqual([
      { id: "u1", title: "First question", summary: "First answer More detail" },
      { id: "u2", title: "", summary: "" },
    ]);
  });

  test("previews and indexed jumps reach unmounted messages with bounded rail DOM", () => {
    const messages=Array.from({length:10000},(_,i)=>({id:`m${i}`,role:i%2?"assistant":"user",text:`Message ${i}`}));
    const turns=createConversationIndex()(messages).turns;
    const listRef=createRef<HTMLDivElement>(); const list=document.createElement("div"), parent=document.createElement("div"); parent.append(list); listRef.current=list;
    const jumped:number[]=[]; const controlRef=createRef<TimelineControl>();controlRef.current={jump:index=>jumped.push(index)};
    const view=render(<ChatNavigation turns={turns} listRef={listRef} controlRef={controlRef} visibleIndex={0} lang="en" onNavigate={()=>{}} />);
    expect(view.container.querySelectorAll('button').length).toBeLessThan(120);
    const button=view.getByRole("button",{name:"Turn 1：Message 0"});fireEvent.focus(button);
    expect(view.getByRole("tooltip").textContent).toContain("Message 1");fireEvent.keyDown(button,{key:"Escape"});expect(view.queryByRole("tooltip")).toBeNull();fireEvent.click(button);expect(jumped).toEqual([0]);
    expect(activeTurnIndex(turns,9999)).toBe(4999);
    view.rerender(<ChatNavigation turns={turns} listRef={listRef} controlRef={controlRef} visibleIndex={9999} lang="en" onNavigate={()=>{}} />);
    expect(view.container.querySelectorAll('button').length).toBeLessThan(120);
    fireEvent.click(view.getByRole("button",{name:"Turn 5000：Message 9998"}));expect(jumped.at(-1)).toBe(9998);
  });
});
