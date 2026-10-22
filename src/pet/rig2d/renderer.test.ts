import { afterEach, expect, test } from 'bun:test';
import source from '../../../public/personas/baobao/figure-rig2d.json';
import { parseRig2dConfig, type Rig2dState, type Vec2 } from './config';
import { Rig2dRenderer, type FrameScheduler } from './renderer';

const contextDescriptor = Object.getOwnPropertyDescriptor(HTMLCanvasElement.prototype, 'getContext');
const dprDescriptor = Object.getOwnPropertyDescriptor(globalThis, 'devicePixelRatio');
const live: Rig2dRenderer[] = [];
afterEach(() => {
  for (const renderer of live.splice(0)) renderer.destroy();
  if (contextDescriptor) Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', contextDescriptor);
  if (dprDescriptor) Object.defineProperty(globalThis, 'devicePixelRatio', dprDescriptor);
  else Reflect.deleteProperty(globalThis, 'devicePixelRatio');
});
type Copy = { destination: HTMLCanvasElement; source: CanvasImageSource; alpha: number; operation: string; args: number[] };
function fixture(idleFps: 30 | 60 = 60) {
  const copies: Copy[] = [], clears: HTMLCanvasElement[] = [], marks: string[] = [];
  const contexts = new WeakMap<HTMLCanvasElement, object>();
  Object.defineProperty(HTMLCanvasElement.prototype, 'getContext', { configurable: true, value(this: HTMLCanvasElement) {
    let context = contexts.get(this);
    if (!context) {
      const canvas = this, saved: { alpha: number; operation: string }[] = [];
      context = {
        globalAlpha: 1, globalCompositeOperation: 'source-over',
        clearRect() { clears.push(canvas); },
        save(this: { globalAlpha: number; globalCompositeOperation: string }) { saved.push({alpha: this.globalAlpha, operation: this.globalCompositeOperation}); },
        restore(this: { globalAlpha: number; globalCompositeOperation: string }) { const previous = saved.pop(); if (previous) { this.globalAlpha=previous.alpha; this.globalCompositeOperation=previous.operation; } },
        drawImage(this: { globalAlpha: number; globalCompositeOperation: string }, source: CanvasImageSource, ...args: number[]) { copies.push({destination:canvas,source,alpha:this.globalAlpha,operation:this.globalCompositeOperation,args}); },
        scale() {}, translate() {}, rotate() {}, strokeText() {}, fillText(text: string) { marks.push(text); },
      };
      contexts.set(this, context);
    }
    return context;
  } });
  let now = 0, nextId = 0, deleted = 0, width = 512, failDraw = false;
  let sourceCanvas!: HTMLCanvasElement;
  const draws: { state: Rig2dState; clock: number; pixels: number; now: number }[] = [], errors: string[] = [];
  const pending = new Map<number, FrameRequestCallback>();
  const scheduler: FrameScheduler = { now: () => now, request: fn => { const id=++nextId; pending.set(id,fn); return id; }, cancel: id => { pending.delete(id); } };
  const polygons: (readonly (readonly Vec2[])[])[] = [];
  const config=parseRig2dConfig(source);
  const canvas=document.createElement('canvas'); Object.defineProperty(canvas,'clientWidth',{get:()=>width});
  const renderer=new Rig2dRenderer(canvas,{config,images:new Map()},p=>polygons.push(p),message=>errors.push(message),scheduler,canvas=>{
    sourceCanvas=canvas;
    return {draw(state,clock,pixels) { if (failDraw) throw new Error('draw failed'); draws.push({state,clock,pixels,now}); },destroy() { deleted++; }};
  },idleFps);
  live.push(renderer);
  const step=(delta:number) => { now+=delta; const callbacks=[...pending.values()]; pending.clear(); for(const fn of callbacks) fn(now); };
  return {renderer,step,pending,draws,copies,clears,marks,polygons,errors,canvas,source:()=>sourceCanvas,deleted:()=>deleted,setWidth:(value:number)=>{width=value;},failDraw:()=>{failDraw=true;}};
}

const replay = (displayHz: number, state: Rig2dState, idleFps: 30 | 60 = 60) => {
  const f=fixture(idleFps); f.renderer.setState(state); f.renderer.setVisible(true); f.step(0);
  for (let frame=0; frame<displayHz*5; frame++) f.step(1000/displayHz);
  return f;
};

test('60/144/240 Hz retain 60 draws/s and the same elapsed animation time; 30 fps stays opt-in', () => {
  const evidence: object[] = [];
  for (const hz of [60,144,240]) for (const state of ['idle','sleep','talking'] as const) for (const idleFps of [60,30] as const) {
    const f=replay(hz,state,idleFps), expected=idleFps===30 && state!=='talking' ? 30 : 60;
    expect(f.draws.length).toBe(expected*5+1);
    expect(f.draws.at(-1)?.clock).toBeCloseTo(5,8);
    expect(f.pending.size).toBe(1); expect(f.errors).toEqual([]);
    evidence.push({displayHz:hz,state,idleFps,seconds:5,draws:f.draws.length,steadyDrawsPerSecond:(f.draws.length-1)/5,lastClock:f.draws.at(-1)?.clock});
    f.renderer.destroy();
  }
  console.log('P11 simulated RAF matrix:',JSON.stringify(evidence));
});

test('rapid state updates cannot bypass the 60 fps draw budget', () => {
  for (const hz of [60,144,240]) {
    const f=fixture(); f.renderer.setVisible(true); f.step(0);
    for (let frame=0;frame<hz*5;frame++) { f.renderer.setState(frame%2 ? 'talking' : 'thinking'); f.step(1000/hz); }
    expect(f.draws.length).toBe(301); expect(f.draws.at(-1)?.clock).toBeCloseTo(5,8);
    f.renderer.destroy();
  }
});

test('skipped RAF time advances animation and a late callback does not trigger catch-up draw bursts', () => {
  const f=fixture(); f.renderer.setVisible(true); f.step(0);
  f.step(4); f.step(4); f.step(4); expect(f.draws.length).toBe(1);
  f.step(5); expect(f.draws.length).toBe(2); expect(f.draws.at(-1)?.clock).toBeCloseTo(.017,10);
  f.step(5000); expect(f.draws.length).toBe(3); expect(f.draws.at(-1)?.clock).toBeCloseTo(.067,10);
  f.step(0); expect(f.draws.length).toBe(3); expect(f.pending.size).toBe(1);
});

test('steady non-sleep frames copy source directly to output and resize with the capped DPR', () => {
  Object.defineProperty(globalThis,'devicePixelRatio',{configurable:true,value:3});
  const f=fixture(); f.setWidth(320); f.renderer.setVisible(true); f.step(0); f.step(1000/60);
  expect(f.draws.map(draw=>draw.pixels)).toEqual([640,640]); expect(f.canvas.width).toBe(640);
  expect(f.copies.length).toBe(2); expect(f.clears).toEqual([f.canvas,f.canvas]);
  for (const copy of f.copies) { expect(copy.destination).toBe(f.canvas); expect(copy.source).toBe(f.source()); expect(copy.alpha).toBe(1); expect(copy.operation).toBe('source-over'); }
  f.setWidth(200); f.step(1000/60); expect(f.canvas.width).toBe(400); expect(f.canvas.height).toBe(400);
  expect(f.copies.at(-1)?.args).toEqual([0,0,400,400]);
});

test('120 ms transitions retain additive blending and return to the direct path at completion', () => {
  const f=fixture(); f.renderer.setVisible(true); f.step(0); f.renderer.setState('thinking');
  expect(f.copies.at(-1)?.source).toBe(f.canvas); f.copies.length=0;
  f.step(60); expect(f.copies.length).toBe(3);
  expect(f.copies[0]?.source).toBe(f.source()); expect(f.copies[0]?.destination).not.toBe(f.canvas);
  expect(f.copies[1]?.destination).toBe(f.canvas); expect(f.copies[1]?.alpha).toBe(.5); expect(f.copies[1]?.operation).toBe('source-over');
  expect(f.copies[2]?.destination).toBe(f.canvas); expect(f.copies[2]?.alpha).toBe(.5); expect(f.copies[2]?.operation).toBe('lighter');
  expect(f.polygons.at(-1)?.length).toBe(2); f.copies.length=0;
  f.step(60); expect(f.copies.length).toBe(1); expect(f.copies[0]?.source).toBe(f.source()); expect(f.copies[0]?.alpha).toBe(1);
  expect(f.copies[0]?.operation).toBe('source-over'); expect(f.polygons.at(-1)?.length).toBe(1);
});

test('sleep keeps its overlay composite and candidate transitions still render at 60 fps', () => {
  const f=fixture(30); f.renderer.setState('sleep'); f.renderer.setVisible(true); f.step(0);
  expect(f.copies.length).toBe(2); expect(f.copies[0]?.source).toBe(f.source()); expect(f.copies[1]?.destination).toBe(f.canvas); expect(f.marks).toEqual(['z','z','z']);
  f.renderer.setState('idle'); const before=f.draws.length;
  for (let frame=0;frame<24;frame++) f.step(1000/240);
  expect(f.draws.length-before).toBe(6); expect(f.polygons.at(-1)?.length).toBe(2);
  for (let frame=0;frame<8;frame++) f.step(1000/240);
  expect(f.polygons.at(-1)?.length).toBe(1);
});

test('hidden renderer schedules no RAF; resume resets elapsed time and retains the remaining fade', () => {
  const f=fixture(); f.renderer.setVisible(true); f.step(0); f.step(20); f.renderer.setState('thinking'); f.step(40);
  const clock=f.draws.at(-1)?.clock, count=f.draws.length;
  f.renderer.setVisible(false); expect(f.pending.size).toBe(0); f.step(10000); expect(f.draws.length).toBe(count);
  f.renderer.setVisible(true); f.renderer.setVisible(true); expect(f.pending.size).toBe(1); f.copies.length=0; f.step(0);
  expect(f.draws.at(-1)?.clock).toBe(clock); expect(f.copies.at(-1)?.alpha).toBeCloseTo(1/3,8); expect(f.polygons.at(-1)?.length).toBe(2);
  f.step(80); expect(f.polygons.at(-1)?.length).toBe(1);
  f.renderer.destroy(); f.renderer.destroy(); expect(f.pending.size).toBe(0); expect(f.deleted()).toBe(1);
  f.renderer.setVisible(true); expect(f.pending.size).toBe(0);
});

test('state changes while hidden start their transition when rendering resumes', () => {
  const f=fixture(); f.renderer.setVisible(true); f.step(0); f.renderer.setVisible(false); f.step(5000);
  f.renderer.setState('thinking'); f.step(5000); f.renderer.setVisible(true); f.copies.length=0; f.step(0);
  expect(f.copies.at(-1)?.alpha).toBe(0); expect(f.draws.at(-1)?.clock).toBe(0); expect(f.polygons.at(-1)?.length).toBe(2);
  f.step(120); expect(f.polygons.at(-1)?.length).toBe(1);
});

test('rapid switches snapshot current blend and retain exact polygon union until final fade', () => {
  const f=fixture(); f.renderer.setVisible(true); f.step(0);
  f.renderer.setState('thinking'); f.step(60); f.renderer.setState('talking');
  expect(f.copies.at(-1)?.source).toBe(f.canvas); expect(f.polygons.at(-1)?.length).toBe(3);
  f.step(60); expect(f.polygons.at(-1)?.length).toBe(3);
  f.step(60); expect(f.polygons.at(-1)?.length).toBe(1); expect(f.draws.at(-1)?.state).toBe('talking');
});

for (const failure of ['context loss','draw error'] as const) test(`${failure} stops scheduling and releases renderer resources exactly once`, () => {
  const f=fixture(); f.renderer.setVisible(true);
  if (failure==='context loss') f.source().dispatchEvent(new Event('webglcontextlost',{cancelable:true}));
  else { f.failDraw(); f.step(0); }
  expect(f.pending.size).toBe(0); expect(f.deleted()).toBe(1); expect(f.errors.length).toBe(1);
  expect(f.errors[0]).toContain(failure==='context loss' ? 'context lost' : 'draw failed');
  f.renderer.setVisible(true); expect(f.pending.size).toBe(0); f.renderer.destroy(); expect(f.deleted()).toBe(1);
});
