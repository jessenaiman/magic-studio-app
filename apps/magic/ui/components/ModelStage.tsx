import React, { useEffect, useRef, useState } from "react";
import * as THREE from "three";
import { GLTFLoader } from "three/addons/loaders/GLTFLoader.js";
import { OrbitControls } from "three/addons/controls/OrbitControls.js";
import { fileUrl } from "../api";

interface Props { path: string; onClip?: (name: string, duration: number) => void }
interface Playback { mixer: THREE.AnimationMixer; clips: THREE.AnimationClip[]; index: number; playing: boolean; time: number; duration: number; speed: number }

function disposeObject(root: THREE.Object3D) {
  const textures = new Set<THREE.Texture>();
  root.traverse(object => {
    if (!(object instanceof THREE.Mesh)) return;
    object.geometry.dispose();
    for (const material of Array.isArray(object.material) ? object.material : [object.material]) {
      for (const value of Object.values(material)) if (value instanceof THREE.Texture) textures.add(value);
      material.dispose();
    }
  });
  for (const texture of textures) { texture.dispose(); const image = texture.source?.data; if (typeof ImageBitmap !== "undefined" && image instanceof ImageBitmap) image.close(); }
}

export const ModelStage: React.FC<Props> = ({ path, onClip }) => {
  const host = useRef<HTMLDivElement>(null);
  const playback = useRef<Playback | null>(null);
  const callback = useRef(onClip); callback.current = onClip;
  const [clips, setClips] = useState<THREE.AnimationClip[]>([]);
  const [index, setIndex] = useState(0);
  const [playing, setPlaying] = useState(false);
  const [time, setTime] = useState(0);
  const [error, setError] = useState("");
  const [loaded, setLoaded] = useState(false);
  const [speed, setSpeed] = useState(1);
  const resetCamera = useRef<() => void>(() => {});

  const selectClip = (next: number) => {
    const state = playback.current; if (!state) return;
    state.mixer.stopAllAction(); state.index = next; state.time = 0;
    const clip = state.clips[next]; state.duration = clip?.duration || 0;
    if (clip) { state.mixer.clipAction(clip).reset().play(); state.mixer.setTime(0); callback.current?.(clip.name, clip.duration); }
    setIndex(next); setTime(0);
  };
  useEffect(() => {
    if (!host.current) return;
    const element = host.current;
    let disposed = false;
    let model: THREE.Object3D | undefined;
    let renderer: THREE.WebGLRenderer;
    setLoaded(false); setClips([]); setError(""); setPlaying(false); setTime(0); setIndex(0); setSpeed(1);
    try { renderer = new THREE.WebGLRenderer({ antialias: true, alpha: false }); }
    catch (cause) { setError(`3D preview could not initialize: ${cause}`); return; }
    renderer.setPixelRatio(Math.min(devicePixelRatio, 2));
    renderer.outputColorSpace = THREE.SRGBColorSpace;
    renderer.toneMapping = THREE.ACESFilmicToneMapping;
    renderer.domElement.id = "animation-model-canvas";
    renderer.domElement.setAttribute("aria-label", "3D character preview. Drag to orbit; scroll to zoom.");
    element.append(renderer.domElement);
    const scene = new THREE.Scene(); scene.background = new THREE.Color("#121923");
    const camera = new THREE.PerspectiveCamera(38, 1, 0.01, 1000);
    const controls = new OrbitControls(camera, renderer.domElement); controls.enableDamping = true;
    scene.add(new THREE.HemisphereLight(0xe8f2ff, 0x53616e, 2.6));
    const key = new THREE.DirectionalLight(0xffedd9, 3.2); key.position.set(3, 5, 4); scene.add(key);
    const rim = new THREE.DirectionalLight(0xb3dbff, 2); rim.position.set(-4, 3, -3); scene.add(rim);
    const resize = () => { const width = Math.max(1, element.clientWidth), height = Math.max(1, element.clientHeight); renderer.setSize(width, height); camera.aspect = width / height; camera.updateProjectionMatrix(); };
    const observer = new ResizeObserver(resize); observer.observe(element); resize();
    void new GLTFLoader().loadAsync(fileUrl(path)).then(gltf => {
      if (disposed) { disposeObject(gltf.scene); return; }
      model = gltf.scene; scene.add(model);
      model.updateMatrixWorld(true);
      const box = new THREE.Box3().setFromObject(model); const center = box.getCenter(new THREE.Vector3()); const size = box.getSize(new THREE.Vector3());
      const radius = Math.max(size.x, size.y, size.z, 0.1);
      resetCamera.current = () => { controls.target.copy(center); camera.position.copy(center).add(new THREE.Vector3(radius * .12, radius * .15, radius * 1.8)); camera.near = radius / 1000; camera.far = radius * 100; camera.updateProjectionMatrix(); controls.update(); };
      resetCamera.current();
      const grid = new THREE.GridHelper(radius * 3, 20, 0x45556b, 0x273443); grid.position.set(center.x, box.min.y, center.z); scene.add(grid);
      const mixer = new THREE.AnimationMixer(model);
      playback.current = { mixer, clips: gltf.animations, index: 0, playing: false, time: 0, duration: gltf.animations[0]?.duration || 0, speed: 1 };
      setClips(gltf.animations); selectClip(0); setLoaded(true);
    }).catch(cause => { if (!disposed) setError(`Could not load this model: ${cause}`); });
    let last = performance.now(), lastUi = last;
    renderer.setAnimationLoop(() => {
      const now = performance.now(), dt = Math.min((now - last) / 1000, .1); last = now;
      const state = playback.current;
      if (state?.playing && state.duration > 0) {
        state.time = (state.time + dt * state.speed) % state.duration; state.mixer.setTime(state.time);
        if (now - lastUi > 80) { setTime(state.time); lastUi = now; }
      }
      controls.update(); renderer.render(scene, camera);
      renderer.domElement.dataset.ready = String(!!model);
      renderer.domElement.dataset.time = String(state?.time || 0);
    });
    return () => {
      disposed = true; renderer.setAnimationLoop(null); observer.disconnect(); controls.dispose();
      playback.current?.mixer.stopAllAction(); if (model) playback.current?.mixer.uncacheRoot(model);
      playback.current = null; disposeObject(scene); renderer.dispose(); renderer.domElement.remove();
    };
  }, [path]);
  const duration = clips[index]?.duration || 0;
  return <div id="animation-model-stage" data-ready={loaded} style={{ display: "flex", flexDirection: "column", width: "100%", height: "100%", minHeight: 340 }}>
    <div style={{ display: "flex", gap: 8, alignItems: "center", padding: 8 }}>
      <select id="animation-clip" aria-label="Animation clip" value={index} disabled={!clips.length} onChange={event => selectClip(Number(event.target.value))} style={{ minWidth: 0, flex: 1 }}>
        {!clips.length && <option value={0}>{loaded ? "Static model — no clips" : "Loading model…"}</option>}
        {clips.map((clip, i) => <option key={`${clip.name}-${i}`} value={i}>{clip.name} · {clip.duration.toFixed(2)}s</option>)}
      </select>
      <button id="animation-camera-reset" className="ghost" onClick={() => resetCamera.current()}>Fit</button>
    </div>
    <div ref={host} style={{ flex: 1, minHeight: 260, overflow: "hidden" }} />
    {error && <p id="animation-model-error" role="alert" style={{ color: "#ed8796", padding: 8 }}>{error}</p>}
    <div style={{ display: "flex", gap: 8, alignItems: "center", padding: 8 }}>
      <button id="animation-play" className="start" disabled={!duration} onClick={() => { if (playback.current) { playback.current.playing = !playing; setPlaying(!playing); } }}>{playing ? "Pause" : "Play"}</button>
      <input id="animation-model-time" aria-label="Animation time" type="range" min={0} max={duration || 1} step={.01} value={time} disabled={!duration} style={{ flex: 1 }}
        onChange={event => { const value = Number(event.target.value); if (playback.current) { playback.current.time = value; playback.current.mixer.setTime(value); } setTime(value); }} />
      <span style={{ whiteSpace: "nowrap", fontSize: 12 }}>{time.toFixed(2)} / {duration.toFixed(2)}s</span>
      <select id="animation-speed" aria-label="Playback speed" value={speed} onChange={event => { const value = Number(event.target.value); setSpeed(value); if (playback.current) playback.current.speed = value; }}>
        {[.25,.5,1,1.5,2].map(value => <option key={value} value={value}>{value}×</option>)}
      </select>
    </div>
  </div>;
};
