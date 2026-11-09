const MARK_COLORS = ["#ff2d55", "#00c2ff", "#ffd400", "#7cff00", "#ff7a00", "#c86bff", "#00ffa8", "#ffffff", "#ff9ad5"];

class CharacterStage {
  stage = null;
  host = null;
  world = null;
  gridLayer = null;
  frameLayer = null;
  marksLayer = null;
  picture = null;
  size = [1024, 1200];
  images = new Map;
  shown = "";
  marks = null;
  onMarks = null;
  mount(host) {
    this.stage?.destroy();
    this.host = host;
    this.images.clear();
    this.shown = "";
    this.stage = new Konva.Stage({ container: host, width: host.clientWidth || 1, height: host.clientHeight || 1 });
    this.gridLayer = new Konva.Layer({ listening: false });
    this.frameLayer = new Konva.Layer({ listening: false });
    this.marksLayer = new Konva.Layer;
    this.stage.add(this.gridLayer, this.frameLayer, this.marksLayer);
    this.picture = new Konva.Image({ x: 0, y: 0 });
    this.frameLayer.add(this.picture);
  }
  setSize(size) {
    if (Array.isArray(size) && size[0] > 0 && size[1] > 0)
      this.size = [size[0], size[1]];
  }
  fit() {
    if (!this.stage || !this.host)
      return;
    const w = this.host.clientWidth, h = this.host.clientHeight;
    if (!w || !h)
      return;
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
    if (!motion.grid)
      return;
    const stroke = getComputedStyle(document.documentElement).getPropertyValue("--line") || "#444";
    const [w, h] = this.size;
    for (let x = 0;x <= w; x += 50)
      this.gridLayer.add(new Konva.Line({ points: [x, 0, x, h], stroke, strokeWidth: 1, strokeScaleEnabled: false }));
    for (let y = 0;y <= h; y += 50)
      this.gridLayer.add(new Konva.Line({ points: [0, y, w, y], stroke, strokeWidth: 1, strokeScaleEnabled: false }));
  }
  load(path) {
    let pending = this.images.get(path);
    if (!pending) {
      pending = new Promise((resolve, reject) => {
        const img = new Image;
        img.onload = () => resolve(img);
        img.onerror = () => reject(new Error(`Could not load ${path.split("/").pop()}`));
        img.src = String(convertFileSrc(path));
      });
      this.images.set(path, pending);
    }
    return pending;
  }
  preload(paths) {
    return Promise.all(paths.map((p) => this.load(p)));
  }
  async show(path) {
    if (!this.stage || !path)
      return;
    this.shown = path;
    const img = await this.load(path);
    if (this.shown !== path)
      return;
    this.picture.image(img);
    this.picture.size({ width: this.size[0], height: this.size[1] });
    this.frameLayer.batchDraw();
  }
  showMarks(parts, editable) {
    this.marksLayer.destroyChildren();
    this.marks = parts ? parts.map((p) => ({ ...p, joint: [...p.joint], outline: p.outline.map((pt) => [...pt]) })) : null;
    for (const [i, part] of (this.marks || []).entries())
      this.drawPart(part, MARK_COLORS[i % MARK_COLORS.length], editable);
    this.marksLayer.batchDraw();
  }
  drawPart(part, color, editable) {
    const outline = new Konva.Line({ points: part.outline.flat(), closed: true, stroke: color, strokeWidth: 2, strokeScaleEnabled: false, fill: `${color}22` });
    this.marksLayer.add(outline);
    const changed = () => {
      outline.points(part.outline.flat());
      this.onMarks?.(this.marks || []);
    };
    if (editable) {
      part.outline.forEach((pt) => {
        const dot = new Konva.Rect({ x: pt[0] - 3, y: pt[1] - 3, width: 6, height: 6, fill: color, draggable: true, strokeScaleEnabled: false });
        dot.on("dragmove", () => {
          pt[0] = Math.round(dot.x() + 3);
          pt[1] = Math.round(dot.y() + 3);
          changed();
        });
        this.marksLayer.add(dot);
      });
    }
    const joint = new Konva.Circle({ x: part.joint[0], y: part.joint[1], radius: 9, fill: color, stroke: "#000", strokeWidth: 1, draggable: editable, strokeScaleEnabled: false });
    joint.on("dragmove", () => {
      part.joint = [Math.round(joint.x()), Math.round(joint.y())];
      this.onMarks?.(this.marks || []);
    });
    const label = new Konva.Text({ x: part.joint[0] + 12, y: part.joint[1] - 8, text: part.id, fill: color, fontSize: 16, listening: false });
    joint.on("dragmove.label", () => label.position({ x: joint.x() + 12, y: joint.y() - 8 }));
    this.marksLayer.add(joint, label);
  }
  clearMarks() {
    this.marksLayer?.destroyChildren();
    this.marks = null;
    this.marksLayer?.batchDraw();
  }
}
const characterStage = new CharacterStage;

class Tripo3DStage {
  host = null;
  renderer = null;
  scene = null;
  camera = null;
  controls = null;
  gridHelper = null;
  modelRoot = null;
  skeletonHelper = null;
  mixer = null;
  activeAction = null;
  animations = [];
  loadedUrl = "";
  skeletonVisible = true;
  wireframe = false;
  duration = 0;
  rafId = 0;
  mount(host) {
    this.dispose();
    this.host = host;
    if (typeof THREE === "undefined" || !THREE.WebGLRenderer)
      return;
    const w = Math.max(1, host.clientWidth || 640);
    const h = Math.max(1, host.clientHeight || 480);
    this.scene = new THREE.Scene;
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
    const hemi = new THREE.HemisphereLight(16777215, 3356738, 1.4);
    const dir1 = new THREE.DirectionalLight(16777215, 1.5);
    dir1.position.set(2.5, 4, 3);
    const dir2 = new THREE.DirectionalLight(9483519, 0.7);
    dir2.position.set(-2.5, 2, -2.5);
    this.gridHelper = new THREE.GridHelper(6, 12, 5258211, 2764345);
    this.scene.add(hemi, dir1, dir2, this.gridHelper);
    if (THREE.OrbitControls) {
      this.controls = new THREE.OrbitControls(this.camera, this.renderer.domElement);
      this.controls.target.set(0, 0.8, 0);
      this.controls.enableDamping = true;
      this.controls.update();
    }
    const loop = () => {
      if (!this.renderer)
        return;
      if (window.__magicWorkspaceVisible === false) { this.rafId = requestAnimationFrame(loop); return; }
      this.controls?.update();
      if (this.gridHelper)
        this.gridHelper.visible = Boolean(motion.grid);
      this.renderer.render(this.scene, this.camera);
      this.rafId = requestAnimationFrame(loop);
    };
    this.rafId = requestAnimationFrame(loop);
  }
  fit() {
    if (!this.renderer || !this.host || !this.camera)
      return;
    const w = Math.max(1, this.host.clientWidth);
    const h = Math.max(1, this.host.clientHeight);
    this.camera.aspect = w / h;
    this.camera.updateProjectionMatrix();
    this.renderer.setSize(w, h);
  }
  frameModel(root) {
    if (!this.camera || !root)
      return;
    const box = new THREE.Box3().setFromObject(root);
    if (box.isEmpty())
      return;
    const size = box.getSize(new THREE.Vector3);
    const center = box.getCenter(new THREE.Vector3);
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
  async loadGlb(path) {
    if (!path || !this.scene || typeof THREE === "undefined" || !THREE.GLTFLoader)
      return false;
    const url = String(convertFileSrc(path));
    if (this.loadedUrl === url && this.modelRoot)
      return true;
    this.clearModel();
    this.loadedUrl = url;
    return new Promise((resolve, reject) => {
      const loader = new THREE.GLTFLoader;
      loader.load(url, (gltf) => {
        if (this.loadedUrl !== url || !this.scene) {
          resolve(false);
          return;
        }
        this.modelRoot = gltf.scene || gltf.scenes?.[0];
        if (!this.modelRoot) {
          resolve(false);
          return;
        }
        this.scene.add(this.modelRoot);
        this.skeletonHelper = new THREE.SkeletonHelper(this.modelRoot);
        this.skeletonHelper.visible = this.skeletonVisible;
        this.scene.add(this.skeletonHelper);
        this.animations = Array.isArray(gltf.animations) ? gltf.animations : [];
        this.mixer = new THREE.AnimationMixer(this.modelRoot);
        this.duration = 0;
        if (this.animations.length > 0)
          this.playClip([this.animations[0].name], 0);
        this.applyWireframe(this.wireframe);
        this.frameModel(this.modelRoot);
        resolve(true);
      }, undefined, error => reject(new Error(`Could not load model ${path}: ${error?.message || error}`)));
    });
  }
  playClip(candidates, yaw) {
    if (!this.mixer || !this.modelRoot)
      return null;
    const name = modelClipMatch(this.animations.map((clip) => clip.name), candidates);
    const clip = this.animations.find((c) => c.name === name);
    if (!clip)
      return null;
    this.mixer.stopAllAction();
    this.activeAction = this.mixer.clipAction(clip);
    this.activeAction.play();
    this.duration = clip.duration || 1;
    this.modelRoot.rotation.y = THREE.MathUtils.degToRad(yaw || 0);
    return this.duration;
  }
  setSkeletonVisible(show) {
    this.skeletonVisible = show;
    if (this.skeletonHelper)
      this.skeletonHelper.visible = show;
  }
  applyWireframe(wire) {
    this.wireframe = wire;
    if (!this.modelRoot)
      return;
    this.modelRoot.traverse((node) => {
      if (node.isMesh && node.material) {
        const mats = Array.isArray(node.material) ? node.material : [node.material];
        for (const m of mats)
          m.wireframe = wire;
      }
    });
  }
  seek(seconds) {
    if (!this.mixer || !this.activeAction)
      return;
    const d = this.duration || 1;
    this.activeAction.paused = false;
    this.mixer.setTime(Math.max(0, Math.min(d, seconds)));
    this.activeAction.paused = true;
  }
  clearModel() {
    if (this.mixer) {
      this.mixer.stopAllAction();
      this.mixer = null;
    }
    if (this.skeletonHelper && this.scene) {
      this.scene.remove(this.skeletonHelper);
      this.skeletonHelper = null;
    }
    if (this.modelRoot) {
      this.modelRoot.traverse((node) => {
        if (node.geometry)
          node.geometry.dispose?.();
        if (node.material) {
          const mats = Array.isArray(node.material) ? node.material : [node.material];
          for (const m of mats) {
            m.map?.dispose?.();
            m.dispose?.();
          }
        }
      });
      if (this.scene)
        this.scene.remove(this.modelRoot);
      this.modelRoot = null;
    }
    this.animations = [];
    this.activeAction = null;
    this.loadedUrl = "";
  }
  dispose() {
    if (this.rafId) {
      cancelAnimationFrame(this.rafId);
      this.rafId = 0;
    }
    this.clearModel();
    this.controls?.dispose?.();
    this.controls = null;
    if (this.renderer?.domElement?.parentElement) {
      this.renderer.domElement.parentElement.replaceChildren();
    }
    this.renderer?.dispose?.();
    this.renderer = null;
    this.scene = null;
    this.camera = null;
    this.host = null;
  }
}
const tripo3DStage = new Tripo3DStage;
