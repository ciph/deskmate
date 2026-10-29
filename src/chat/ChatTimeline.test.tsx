import { virtualLayout } from "../testing/virtualLayout";
import { beforeEach, afterEach, expect, test } from "bun:test";
import { cleanup, render, fireEvent } from "@testing-library/react";
import { createRef } from "react";
import { ChatTimeline, type TimelineControl } from "./ChatTimeline";
import { retainedRange } from "./timelineRange";
let restoreLayout = () => {};
beforeEach(() => { restoreLayout = virtualLayout(); });
afterEach(() => { cleanup(); restoreLayout(); });
test("range has eight-row overscan plus explicit focus/media/selection only", () => {
  const range = { startIndex: 100, endIndex: 105, overscan: 8, count: 10000 };
  const normal = retainedRange(range, new Set(), null, false);
  expect(normal.length).toBe(22); expect(normal[0]).toBe(92); expect(normal.at(-1)).toBe(113);
  expect(retainedRange(range, new Set([0,9999]), [50,52], false).length).toBe(27);
  expect(retainedRange(range, new Set(), null, true).length).toBe(10000);
});
test("1000 and 10000 rows stay bounded, preserve focused DOM and keep logical positions", () => {
  const listRef=createRef<HTMLDivElement>(),controlRef=createRef<TimelineControl>(),followRef={current:false};
  const ids=Array.from({length:10000},(_,i)=>`row-${i}`);
  const props={listRef,controlRef,followRef,geometry:"en:dark",onVisible:()=>{},renderRow:(index:number)=><pre tabIndex={0}>{index}</pre>};
  const view=render(<ChatTimeline {...props} ids={ids.slice(0,1000)} />);
  expect(view.container.querySelectorAll('[data-timeline-index]').length).toBeLessThan(120);
  const code=view.container.querySelector('pre')!; code.focus(); fireEvent.focusIn(code);
  view.rerender(<ChatTimeline {...props} ids={ids} />);
  expect(view.container.querySelectorAll('[data-timeline-index]').length).toBeLessThan(120);
  expect(listRef.current?.dataset.logicalRows).toBe("10000");
  expect(document.activeElement).toBe(code);
  expect(controlRef.current).not.toBeNull();
});
