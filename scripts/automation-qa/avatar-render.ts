/** Authoring-only: export static portraits from the same models as the pet. */
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { personaById } from "../../src/pet/personaCatalog";
import { personaAssets } from "../../src/pet/personaAssets";
import { applyPersonaTextures } from "../../src/pet/PetRendererResources";
import { PetAnimation } from "../../src/pet/PetAnimation";

const id = new URLSearchParams(location.search).get("persona") ?? "xiaozhu";
const persona = personaById(id);
const canvas = document.querySelector("canvas")!;
const renderer = new THREE.WebGLRenderer({ canvas, alpha: true, antialias: true, preserveDrawingBuffer: true });
renderer.setSize(128, 128);
renderer.outputColorSpace = THREE.SRGBColorSpace;
const scene = new THREE.Scene();
scene.add(new THREE.HemisphereLight(0xffffff, 0x536070, 1.8));
const light = new THREE.DirectionalLight(0xffffff, 2.5);
light.position.set(2, 4, 4);
scene.add(light);
const fill = new THREE.DirectionalLight(0xaecbff, 0.8);
fill.position.set(-3, 2, 2);
scene.add(fill);
const assets = await personaAssets(persona.id);
if (assets.renderType !== "glb") throw new Error("Only bundled GLB avatars are exported here");
const gltf = await new GLTFLoader().loadAsync(assets.modelUrl);
const model = gltf.scene;
if (!persona.embeddedMaterials) await applyPersonaTextures(model, assets);
const animation = new PetAnimation(model, gltf.animations, persona);
animation.setMood("idle");
animation.update(0);
scene.add(model);
model.updateMatrixWorld(true);
const bounds = new THREE.Box3();
model.traverseVisible(object => {
  if (object instanceof THREE.Mesh) bounds.union(new THREE.Box3().setFromObject(object));
});
const size = bounds.getSize(new THREE.Vector3());
const center = bounds.getCenter(new THREE.Vector3());
// Keep the upper body and face prominent at the final 32px display size.
const extent = size.y * 0.72;
const targetY = bounds.max.y - extent * 0.47;
const camera = new THREE.OrthographicCamera(-extent / 2, extent / 2, extent / 2, -extent / 2, 0.001, size.y * 100);
camera.position.set(center.x, targetY, bounds.max.z + size.y * 3);
camera.lookAt(center.x, targetY, center.z);
renderer.render(scene, camera);
document.body.dataset.avatar = canvas.toDataURL("image/png");
animation.dispose();
renderer.dispose();
