// The Animation canvas. Konva (npm `konva`, bundled to vendor/konva.bundle.js) shows the frames
// EffectCraft rendered and the marks a person reviews before PhotoCraft cuts the parts. It draws
// no motion of its own: every pose on screen came from `character.render` / `character.frame`.
declare const Konva: any;
declare const THREE: any;
const MARK_COLORS = ['#ff2d55', '#00c2ff', '#ffd400', '#7cff00', '#ff7a00', '#c86bff', '#00ffa8', '#ffffff', '#ff9ad5'];

class CharacterStage {
  stage: any = null;
  host: HTMLElement | null = null;
  world: any = null;
  gridLayer: any = null;
  frameLayer: any = null;
  marksLayer: any = null;
  picture: any = null;
  size: [number, number] = [1024, 1200];
  images = new Map<string, Promise<HTMLImageElement>>();
  shown = '';
  marks: any[] | null = null;
  onMarks: ((parts: any[]) => void) | null = null;

  mount(host: HTMLElement) {
    this.stage?.destroy();
    this.host = host;
    this.images.clear();
    this.shown = '';
    this.stage = new Konva.Stage({ container: host, width: host.clientWidth || 1, height: host.clientHeight || 1 });
    this.gridLayer = new Konva.Layer({ listening: false });
    this.frameLayer = new Konva.Layer({ listening: false });
    this.marksLayer = new Konva.Layer();
    this.stage.add(this.gridLayer, this.frameLayer, this.marksLayer);
    this.picture = new Konva.Image({ x: 0, y: 0 });
    this.frameLayer.add(this.picture);
  }

  /** The drawing's pixel size; frames rendered at another resolution are scaled to it. */
  setSize(size: any) {
    if (Array.isArray(size) && size[0] > 0 && size[1] > 0) this.size = [size[0], size[1]];
  }

  /** Fit the drawing in the stage (or 1:1), then apply the toolbar zoom. */
  fit() {
    if (!this.stage || !this.host) return;
    const w = this.host.clientWidth, h = this.host.clientHeight;
    if (!w || !h) return;
    this.stage.size({ width: w, height: h });
    const [dw, dh] = this.size;
    const scale = (motion.fit ? Math.min((w - 64) / dw, (h - 64) / dh) : 1) * motion.zoom;
    const at = { x: (w - dw * scale) / 2, y: (h - dh * scale) / 2 };
    for (const layer of [this.gridLayer, this.frameLayer, this.marksLayer]) {
      layer.scale({ x: scale, y: scale });
      layer.position(at);
    }
    this.drawGrid();
    this.stage.batchDraw();
  }

  drawGrid() {
    this.gridLayer.destroyChildren();
    if (!motion.grid) return;
    const stroke = getComputedStyle(document.documentElement).getPropertyValue('--line') || '#444';
    const [w, h] = this.size;
    for (let x = 0; x <= w; x += 50) this.gridLayer.add(new Konva.Line({ points: [x, 0, x, h], stroke, strokeWidth: 1, strokeScaleEnabled: false }));
    for (let y = 0; y <= h; y += 50) this.gridLayer.add(new Konva.Line({ points: [0, y, w, y], stroke, strokeWidth: 1, strokeScaleEnabled: false }));
  }

  load(path: string): Promise<HTMLImageElement> {
    let pending = this.images.get(path);
    if (!pending) {
      pending = new Promise((resolve, reject) => {
        const img = new Image();
        img.onload = () => resolve(img);
        img.onerror = () => reject(new Error(`Could not load ${path.split('/').pop()}`));
        img.src = String(convertFileSrc(path));
      });
      this.images.set(path, pending);
    }
    return pending;
  }

  /** Load every frame of a rendered clip before playback, so playing never waits on a file. */
  preload(paths: string[]) { return Promise.all(paths.map((p) => this.load(p))); }

  async show(path: string) {
    if (!this.stage || !path) return;
    this.shown = path;
    const img = await this.load(path);
    if (this.shown !== path) return;
    this.picture.image(img);
    this.picture.size({ width: this.size[0], height: this.size[1] });
    this.frameLayer.batchDraw();
  }

  /** Outlines and joints for review. Dragging a joint or an outline point edits the marks. */
  showMarks(parts: any[] | null, editable: boolean) {
    this.marksLayer.destroyChildren();
    this.marks = parts ? parts.map((p) => ({ ...p, joint: [...p.joint], outline: p.outline.map((pt: number[]) => [...pt]) })) : null;
    for (const [i, part] of (this.marks || []).entries()) this.drawPart(part, MARK_COLORS[i % MARK_COLORS.length], editable);
    this.marksLayer.batchDraw();
  }

  drawPart(part: any, color: string, editable: boolean) {
    const outline = new Konva.Line({ points: part.outline.flat(), closed: true, stroke: color, strokeWidth: 2, strokeScaleEnabled: false, fill: `${color}22` });
    this.marksLayer.add(outline);
    const changed = () => { outline.points(part.outline.flat()); this.onMarks?.(this.marks || []); };
    if (editable) {
      part.outline.forEach((pt: number[]) => {
        const dot = new Konva.Rect({ x: pt[0] - 3, y: pt[1] - 3, width: 6, height: 6, fill: color, draggable: true, strokeScaleEnabled: false });
        dot.on('dragmove', () => { pt[0] = Math.round(dot.x() + 3); pt[1] = Math.round(dot.y() + 3); changed(); });
        this.marksLayer.add(dot);
      });
    }
    const joint = new Konva.Circle({ x: part.joint[0], y: part.joint[1], radius: 9, fill: color, stroke: '#000', strokeWidth: 1, draggable: editable, strokeScaleEnabled: false });
    joint.on('dragmove', () => { part.joint = [Math.round(joint.x()), Math.round(joint.y())]; this.onMarks?.(this.marks || []); });
    const label = new Konva.Text({ x: part.joint[0] + 12, y: part.joint[1] - 8, text: part.id, fill: color, fontSize: 16, listening: false });
    joint.on('dragmove.label', () => label.position({ x: joint.x() + 12, y: joint.y() - 8 }));
    this.marksLayer.add(joint, label);
  }

  clearMarks() { this.marksLayer?.destroyChildren(); this.marks = null; this.marksLayer?.batchDraw(); }
}
const characterStage = new CharacterStage();

class Tripo3DStage {
  host: HTMLElement | null = null;
  renderer: any = null;
  scene: any = null;
  camera: any = null;
  controls: any = null;
  gridHelper: any = null;
  modelRoot: any = null;
  skeletonHelper: any = null;
  mixer: any = null;
  activeAction: any = null;
  animations: any[] = [];
  loadedUrl = '';
  skeletonVisible = true;
  wireframe = false;
  duration = 0;
  rafId = 0;

  mount(host: HTMLElement) {
    this.dispose();
    this.host = host;
    if (typeof THREE === 'undefined' || !THREE.WebGLRenderer) return;
    const w = Math.max(1, host.clientWidth || 640);
    const h = Math.max(1, host.clientHeight || 480);
    this.scene = new THREE.Scene();
    this.camera = new THREE.PerspectiveCamera(42, w / h, 0.01, 100);
    this.camera.position.set(0, 1.3, 2.8);
    try {
      this.renderer = new THREE.WebGLRenderer({ antialias: true, alpha: true });
    } catch {
      this.renderer = null;
      return;
    }
    this.renderer.setPixelRatio(Math.min(window.devicePixelRatio || 1, 2));
    this.renderer.setSize(w, h);
    host.replaceChildren(this.renderer.domElement);
    const hemi = new THREE.HemisphereLight(0xffffff, 0x333842, 1.4);
    const dir1 = new THREE.DirectionalLight(0xffffff, 1.5);
    dir1.position.set(2.5, 4.0, 3.0);
    const dir2 = new THREE.DirectionalLight(0x90b4ff, 0.7);
    dir2.position.set(-2.5, 2.0, -2.5);
    this.gridHelper = new THREE.GridHelper(6, 12, 0x503be3, 0x2a2e39);
    this.scene.add(hemi, dir1, dir2, this.gridHelper);
    if (THREE.OrbitControls) {
      this.controls = new THREE.OrbitControls(this.camera, this.renderer.domElement);
      this.controls.target.set(0, 0.8, 0);
      this.controls.enableDamping = true;
      this.controls.update();
    }
    const loop = () => {
      if (!this.renderer) return;
      this.controls?.update();
      if (this.gridHelper) this.gridHelper.visible = Boolean(motion.grid);
      this.renderer.render(this.scene, this.camera);
      this.rafId = requestAnimationFrame(loop);
    };
    this.rafId = requestAnimationFrame(loop);
  }

  fit() {
    if (!this.renderer || !this.host || !this.camera) return;
    const w = Math.max(1, this.host.clientWidth);
    const h = Math.max(1, this.host.clientHeight);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }

  frameModel(root: any) {
    if (!this.camera || !root) return;
    const box = new THREE.Box3().setFromObject(root);
    if (box.isEmpty()) return;
    const size = box.getSize(new THREE.Vector3());
    const center = box.getCenter(new THREE.Vector3());
    const maxDim = Math.max(size.x, size.y, size.z) || 1;
    const dist = maxDim * 1.85 / Math.max(0.25, motion.zoom || 1);
    this.camera.position.set(center.x + dist * 0.35, center.y + maxDim * 0.35, center.z + dist);
    if (this.controls) {
      this.controls.target.copy(center);
      this.controls.update();
    } else {
      this.camera.lookAt(center);
    }
  }

  async loadGlb(path: string): Promise<boolean> {
    if (!path || !this.scene || typeof THREE === 'undefined' || !THREE.GLTFLoader) return false;
    const url = String(convertFileSrc(path));
    if (this.loadedUrl === url && this.modelRoot) return true;
    this.clearModel();
    this.loadedUrl = url;
    return new Promise((resolve) => {
      const loader = new THREE.GLTFLoader();
      loader.load(url, (gltf: any) => {
        if (this.loadedUrl !== url || !this.scene) { resolve(false); return; }
        this.modelRoot = gltf.scene || gltf.scenes?.[0];
        if (!this.modelRoot) { resolve(false); return; }
        this.scene.add(this.modelRoot);
        this.skeletonHelper = new THREE.SkeletonHelper(this.modelRoot);
        this.skeletonHelper.visible = this.skeletonVisible;
        this.scene.add(this.skeletonHelper);
        this.animations = Array.isArray(gltf.animations) ? gltf.animations : [];
        this.mixer = new THREE.AnimationMixer(this.modelRoot);
        if (this.animations.length > 0) {
          this.activeAction = this.mixer.clipAction(this.animations[0]);
          this.activeAction.play();
          this.duration = this.animations[0].duration || 2;
        } else {
          this.duration = 0;
        }
        this.applyWireframe(this.wireframe);
        this.frameModel(this.modelRoot);
        resolve(true);
      }, undefined, () => resolve(false));
    });
  }

  setSkeletonVisible(show: boolean) {
    this.skeletonVisible = show;
    if (this.skeletonHelper) this.skeletonHelper.visible = show;
  }

  applyWireframe(wire: boolean) {
    this.wireframe = wire;
    if (!this.modelRoot) return;
    this.modelRoot.traverse((node: any) => {
      if (node.isMesh && node.material) {
        const mats = Array.isArray(node.material) ? node.material : [node.material];
        for (const m of mats) m.wireframe = wire;
      }
    });
  }

  seek(seconds: number) {
    if (!this.mixer || !this.activeAction) return;
    const d = this.duration || 1;
    this.activeAction.paused = false;
    this.mixer.setTime(Math.max(0, Math.min(d, seconds)));
    this.activeAction.paused = true;
  }

  clearModel() {
    if (this.mixer) { this.mixer.stopAllAction(); this.mixer = null; }
    if (this.skeletonHelper && this.scene) { this.scene.remove(this.skeletonHelper); this.skeletonHelper = null; }
    if (this.modelRoot && this.scene) { this.scene.remove(this.modelRoot); this.modelRoot = null; }
    this.animations = [];
    this.activeAction = null;
    this.loadedUrl = '';
  }

  dispose() {
    if (this.rafId) { cancelAnimationFrame(this.rafId); this.rafId = 0; }
    this.clearModel();
    this.controls?.dispose?.();
    this.controls = null;
    this.renderer?.dispose?.();
    this.renderer = null;
    this.scene = null;
    this.camera = null;
  }
}

const tripo3DStage = new Tripo3DStage();
