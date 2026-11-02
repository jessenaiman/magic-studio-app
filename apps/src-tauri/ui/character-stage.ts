// The Animation canvas. Konva (npm `konva`, bundled to vendor/konva.bundle.js) shows the frames
// EffectCraft rendered and the marks a person reviews before PhotoCraft cuts the parts. It draws
// no motion of its own: every pose on screen came from `character.render` / `character.frame`.
declare const Konva: any;

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
