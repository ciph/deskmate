import { expect, test } from "bun:test";
import { resolve } from "node:path";
import * as THREE from "three";
import { GLTFLoader } from "three/examples/jsm/loaders/GLTFLoader.js";
import { personaById } from "./personaCatalog";
import { variantAnimationBounds } from "./PetAnimation";
import { VariantEnvelopeCache, variantModelSha256 } from "./variantEnvelope";
import { PetRendererView } from "./PetRendererView";
import { disposeObject } from "./PetRendererResources";

const file = Bun.file(resolve(import.meta.dir, "../../public/personas/xiaozhu-sandaime/figure.glb"));

test("bundled envelope skips all 175 skin scans and matches production sampling without changing pose", async () => {
  const loader = new GLTFLoader();
  loader.register(() => ({ name: "cpu_envelope_texture_fixture", loadTexture: async () => new THREE.Texture() }));
  const bytes = await file.arrayBuffer();
  const hash = await variantModelSha256(bytes);
  const gltf = await loader.parseAsync(bytes, "");
  const persona = personaById("xiaozhu-sandaime");
  if (persona.clipRoots === undefined || hash === null) throw new Error("Missing built-in identity");
  const config = { id: persona.id, scale: persona.scale, clipRoots: persona.clipRoots };
  gltf.scene.scale.setScalar(config.scale);
  let scans = 0;
  const skins: THREE.SkinnedMesh[] = [];
  gltf.scene.traverse((node) => {
    if (node instanceof THREE.SkinnedMesh) {
      skins.push(node);
      const compute = node.computeBoundingBox.bind(node);
      node.computeBoundingBox = () => { scans += 1; compute(); };
    }
  });
  try {
    const cache = new VariantEnvelopeCache();
    const trusted = cache.resolve(gltf.scene, gltf.animations, config, hash);
    expect(trusted.source).toBe("builtin");
    expect(scans).toBe(0);
    const sampled = variantAnimationBounds(gltf.scene, gltf.animations, config.clipRoots);
    expect(scans).toBe(175);
    expect(trusted.bounds.min.distanceTo(sampled.min)).toBeLessThan(1e-10);
    expect(trusted.bounds.max.distanceTo(sampled.max)).toBeLessThan(1e-10);
    expect(skins.every((mesh) => mesh.boundingBox === null)).toBe(true);
    expect(cache.resolve(gltf.scene, gltf.animations, config, hash).source).toBe("builtin");
    expect(scans).toBe(175);

    // Independently check every authored key time against the real camera's
    // framing margin. Default and maximum window sizes share this aspect ratio.
    const camera = new THREE.PerspectiveCamera(28, 320 / 420, 0.01, 100);
    PetRendererView.prototype.frameCamera.call({ camera } as PetRendererView, trusted.bounds);
    camera.updateMatrixWorld(true);
    const frustum = new THREE.Frustum().setFromProjectionMatrix(new THREE.Matrix4().multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse));
    const mixer = new THREE.AnimationMixer(gltf.scene);
    let checkedTimes = 0;
    try {
      for (const clip of gltf.animations) {
        const root = gltf.scene.getObjectByName(config.clipRoots[clip.name]);
        if (root === undefined) throw new Error(`Missing root: ${clip.name}`);
        const action = mixer.clipAction(clip).reset().setLoop(THREE.LoopOnce, 1).play();
        action.clampWhenFinished = true;
        const times = new Set([0, clip.duration, ...clip.tracks.flatMap((track) => [...track.times])]);
        for (const time of [...times].sort((a, b) => a - b)) {
          mixer.setTime(time);
          gltf.scene.updateMatrixWorld(true);
          root.traverse((node) => { if (node instanceof THREE.SkinnedMesh) { node.skeleton.update(); node.computeBoundingBox(); } });
          const bounds = new THREE.Box3().setFromObject(root);
          for (const x of [bounds.min.x, bounds.max.x]) for (const y of [bounds.min.y, bounds.max.y]) for (const z of [bounds.min.z, bounds.max.z]) {
            expect(frustum.containsPoint(new THREE.Vector3(x, y, z)), `${clip.name} at ${time}`).toBe(true);
          }
          checkedTimes += 1;
        }
        mixer.stopAllAction();
      }
      expect(checkedTimes).toBeGreaterThan(175);
      console.log(JSON.stringify({ envelope: "xiaozhu-sandaime", hash, productionScans: 175, trustedScans: 0, authoredTimesChecked: checkedTimes, aspect: camera.aspect }));
    } finally { mixer.stopAllAction(); mixer.uncacheRoot(gltf.scene); }
  } finally { disposeObject(gltf.scene); }
}, 60_000);
