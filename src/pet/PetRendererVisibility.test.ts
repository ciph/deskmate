import { afterEach, beforeEach, expect, mock, spyOn, test } from "bun:test";
import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";
import * as originalView from "./PetRendererView";
import { PetRenderer } from "./PetRenderer";

const realView = { ...originalView };
let loop: (() => void) | null = null;
let now = 0;
let draws = 0;
const deltas: number[] = [];
const loopChanges: boolean[] = [];
class GpuBoundary {
  readonly scene = new THREE.Scene();
  readonly camera = new THREE.PerspectiveCamera();
  readonly baseModelRotation = new THREE.Euler();
  readonly renderer = {
    setAnimationLoop(callback: (() => void) | null) { loop = callback; loopChanges.push(callback !== null); },
    render() { draws += 1; }, dispose() {}, forceContextLoss() {},
  };
  frameCamera() {}
  resize() {}
  updateMouseFollow(_model: THREE.Object3D | null, delta: number) { deltas.push(delta); }
  setMouseFollowEnabled() {}
  setMouseTarget() {}
}
function asset(): GLTF {
  const scene = new THREE.Group();
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicMaterial()));
  return { scene, scenes: [scene], animations: [], cameras: [], asset: { version: "2.0" }, userData: {}, parser: {} as GLTF["parser"] };
}
let clock: ReturnType<typeof spyOn<typeof performance, "now">>;
let load: ReturnType<typeof spyOn<GLTFLoader, "loadAsync">>;
beforeEach(() => {
  loop = null; now = 0; draws = 0; deltas.length = 0; loopChanges.length = 0;
  clock = spyOn(performance, "now").mockImplementation(() => now);
  load = spyOn(GLTFLoader.prototype, "loadAsync").mockImplementation(async () => asset());
  mock.module("./PetRendererView", () => ({ ...realView, PetRendererView: GpuBoundary }));
});
afterEach(() => { clock.mockRestore(); load.mockRestore(); mock.module("./PetRendererView", () => realView); });
const advance = (milliseconds: number) => { now += milliseconds; loop?.(); };

test("hidden GLB detaches its animation loop and resumes without a wall-time jump", async () => {
  const renderer = new PetRenderer(document.createElement("canvas"));
  await renderer.load("xiaozhu-nidaime");
  advance(20);
  expect(draws).toBe(1);
  expect(deltas).toEqual([0.02]);
  const oldCallback = loop;
  renderer.setVisible(false);
  expect(loop).toBeNull();
  now += 60_000;
  oldCallback?.();
  expect(draws).toBe(1);
  renderer.setVisible(true);
  advance(20);
  expect(draws).toBe(2);
  expect(deltas).toEqual([0.02, 0.02]);
  expect(loopChanges).toEqual([true, false, true]);
  renderer.dispose();
  expect(loop).toBeNull();
});

test("loading and repeated hide while hidden do not start an animation loop", async () => {
  const renderer = new PetRenderer(document.createElement("canvas"));
  renderer.setVisible(false);
  await renderer.load("xiaozhu-nidaime");
  renderer.setVisible(false);
  advance(90_000);
  expect(loop).toBeNull();
  expect(draws).toBe(0);
  expect(loopChanges).toEqual([false]);
  renderer.setVisible(true);
  advance(20);
  expect(draws).toBe(1);
  expect(deltas).toEqual([0.02]);
  renderer.dispose();
  renderer.setVisible(true);
  expect(loop).toBeNull();
});

test("hide discards a partial gated frame instead of advancing it after resume", async () => {
  const renderer = new PetRenderer(document.createElement("canvas"));
  await renderer.load("xiaozhu-nidaime");
  advance(10);
  expect(draws).toBe(0);
  renderer.setVisible(false);
  now += 30_000;
  renderer.setVisible(true);
  advance(10);
  expect(draws).toBe(0);
  advance(10);
  expect(deltas).toEqual([0.02]);
  renderer.dispose();
});
