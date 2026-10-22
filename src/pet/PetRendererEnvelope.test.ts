import { afterEach, beforeEach, expect, mock, spyOn, test } from "bun:test";
import * as THREE from "three";
import { GLTFLoader, type GLTF } from "three/examples/jsm/loaders/GLTFLoader.js";
import * as originalView from "./PetRendererView";
import { PetRenderer } from "./PetRenderer";

const realView = { ...originalView };
let view: GpuBoundary;
class GpuBoundary {
  readonly scene = new THREE.Scene();
  readonly camera = new THREE.PerspectiveCamera();
  readonly baseModelRotation = new THREE.Euler();
  readonly renderer = { setAnimationLoop() {}, render() {}, dispose() {}, forceContextLoss() {} };
  constructor() { view = this; }
  frameCamera() {} resize() {} updateMouseFollow() {} setMouseFollowEnabled() {} setMouseTarget() {}
}
function asset(): GLTF {
  const scene = new THREE.Group();
  scene.add(new THREE.Mesh(new THREE.BoxGeometry(), new THREE.MeshBasicMaterial()));
  return { scene, scenes: [scene], animations: [], cameras: [], asset: { version: "2.0" }, userData: {}, parser: {} as GLTF["parser"] };
}
function deferred<T>() { let resolve!: (value: T) => void; const promise = new Promise<T>((done) => { resolve = done; }); return { promise, resolve }; }
let file: ReturnType<typeof spyOn<THREE.FileLoader, "loadAsync">>;
let parse: ReturnType<typeof spyOn<GLTFLoader, "parseAsync">>;
let load: ReturnType<typeof spyOn<GLTFLoader, "loadAsync">>;
beforeEach(() => {
  file = spyOn(THREE.FileLoader.prototype, "loadAsync");
  parse = spyOn(GLTFLoader.prototype, "parseAsync");
  load = spyOn(GLTFLoader.prototype, "loadAsync");
  mock.module("./PetRendererView", () => ({ ...realView, PetRendererView: GpuBoundary }));
});
afterEach(() => { file.mockRestore(); parse.mockRestore(); load.mockRestore(); mock.module("./PetRendererView", () => realView); });

test("variant loader parses the single fetched buffer with its original resource base", async () => {
  const bytes = new ArrayBuffer(4);
  const gltf = asset();
  file.mockResolvedValue(bytes);
  parse.mockResolvedValue(gltf);
  const renderer = new PetRenderer(document.createElement("canvas"));
  await renderer.load("xiaozhu-sandaime");
  expect(file).toHaveBeenCalledTimes(1);
  expect(file.mock.calls[0][0]).toBe("/personas/xiaozhu-sandaime/figure.glb");
  expect(parse.mock.calls[0][0]).toBe(bytes);
  expect(parse.mock.calls[0][1]).toBe("/personas/xiaozhu-sandaime/");
  expect(load).not.toHaveBeenCalled();
  expect(view.scene.children).toContain(gltf.scene);
  renderer.dispose();
  expect(view.scene.children).toHaveLength(0);
});

test("a disposed variant load releases its late model without attaching it", async () => {
  const pending = deferred<GLTF>();
  const gltf = asset();
  const geometry = (gltf.scene.children[0] as THREE.Mesh).geometry;
  const dispose = spyOn(geometry, "dispose");
  file.mockResolvedValue(new ArrayBuffer(4));
  parse.mockReturnValue(pending.promise);
  const renderer = new PetRenderer(document.createElement("canvas"));
  const loading = renderer.load("xiaozhu-sandaime");
  // personaAssets and FileLoader each yield; wait until parsing is in flight.
  for (let turn = 0; turn < 5 && parse.mock.calls.length === 0; turn += 1) await Promise.resolve();
  expect(parse).toHaveBeenCalledTimes(1);
  renderer.dispose();
  pending.resolve(gltf);
  await loading;
  expect(dispose).toHaveBeenCalledTimes(1);
  expect(view.scene.children).toHaveLength(0);
  dispose.mockRestore();
});

test("an older variant parse cannot replace a more recently selected persona", async () => {
  const pending = deferred<GLTF>();
  const old = asset(); const current = asset();
  const dispose = spyOn((old.scene.children[0] as THREE.Mesh).geometry, "dispose");
  file.mockResolvedValue(new ArrayBuffer(4)); parse.mockReturnValue(pending.promise); load.mockResolvedValue(current);
  const renderer = new PetRenderer(document.createElement("canvas"));
  const oldLoading = renderer.load("xiaozhu-sandaime");
  await renderer.load("xiaozhu-nidaime");
  pending.resolve(old);
  await oldLoading;
  expect(view.scene.children).toEqual([current.scene]);
  expect(dispose).toHaveBeenCalledTimes(1);
  renderer.dispose(); dispose.mockRestore();
});
