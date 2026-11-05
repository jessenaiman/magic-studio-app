// Composite mode of the Animation view: images stacked on a canvas, each entering and leaving with
// a kind, duration, delay and ease — rendered by the engine (`sequence.render` → PhotoCraft layers
// → EffectCraft track). Subject mode animates one subject's rig; this mode arranges stills in time.
const SEQ_PRESETS: Record<string, number[]> = {
  "1:1": [1080, 1080], "16:9": [1920, 1080], "9:16": [1080, 1920],
  "4:5": [1080, 1350], "3:2": [1500, 1000],
};
const SEQ_KINDS = ["none", "fade", "rise", "drop", "left", "right", "zoom_in", "zoom_out", "turn"];
const SEQ_EASES = ["smooth", "linear", "in", "out"];

function seqEase(name: string, u: number) {
  u = Math.max(0, Math.min(1, u));
  if (name === "linear") return u;
  if (name === "in") return u * u * u;
  if (name === "out") return 1 - (1 - u) ** 3;
  return u * u * (3 - 2 * u);
}

// The resting state and the departing state of one side, in the motion spec's units.
function seqSideValues(kind: string, [w, h]: number[]) {
  const rest = {position: [0, 0], rotation: 0, scale: [0, 0], opacity: 0};
  const travel = 0.12;
  switch (kind) {
    case "fade": return {...rest, opacity: -100};
    case "rise": return {...rest, position: [0, h * travel], opacity: -100};
    case "drop": return {...rest, position: [0, -h * travel], opacity: -100};
    case "left": return {...rest, position: [w * travel, 0], opacity: -100};
    case "right": return {...rest, position: [-w * travel, 0], opacity: -100};
    case "zoom_in": return {...rest, scale: [-25, -25], opacity: -100};
    case "zoom_out": return {...rest, scale: [25, 25], opacity: -100};
    case "turn": return {...rest, rotation: -18, opacity: -100};
    default: return rest;
  }
}

function seqLerp(a: any, b: any, u: number) {
  return {
    position: [a.position[0] + (b.position[0] - a.position[0]) * u, a.position[1] + (b.position[1] - a.position[1]) * u],
    rotation: a.rotation + (b.rotation - a.rotation) * u,
    scale: [a.scale[0] + (b.scale[0] - a.scale[0]) * u, a.scale[1] + (b.scale[1] - a.scale[1]) * u],
    opacity: a.opacity + (b.opacity - a.opacity) * u,
  };
}

function seqField(label: string, control: any) {
  return el("label", {class: "seq-field"}, el("span", {}, label), control);
}

function seqSelect(label: string, options: string[], value: string, change: (value: string) => void) {
  const select = el("select", {"aria-label": label, title: label},
    ...options.map((o) => el("option", {value: o}, o.replace("_", " "))));
  select.value = value;
  select.addEventListener("change", () => change(select.value));
  return seqField(label, select);
}

class SequenceViewController {
  background = "#101820";
  built = false;
  busy = false;
  canvas = [1080, 1080];
  duration = 4;
  fps = 30;
  layers: any[] = [];
  playing = false;
  preset = "1:1";
  project: string | null = null;
  result: any = null;
  selected: string | null = null;
  tick: any = null;
  time = 0;

  sideOffset(spec: any, entering: boolean, t: number, rest: any) {
    if (!spec || spec.kind === "none") return null;
    const start = spec.delay ?? (entering ? 0 : Math.max(0, this.duration - (spec.duration ?? 0.5)));
    const end = Math.min(this.duration, start + (spec.duration ?? 0.6));
    if (t <= start) return null;
    const from = seqSideValues(spec.kind, this.canvas);
    const part = seqLerp(from, rest, seqEase(spec.ease || "smooth", end > start ? (t - start) / (end - start) : 1));
    return entering ? (t >= end ? rest : part) : part;
  }

  offsets(layer: any, t: number) {
    const rest = seqSideValues("none", this.canvas);
    let value = rest;
    for (const [spec, entering] of [[layer.entry, true], [layer.exit, false]] as [any, boolean][]) {
      const off = this.sideOffset(spec, entering, t, rest);
      if (off) value = off;
    }
    return value;
  }

  attach() {
    animationShell();
    const stage = $("motion-stage");
    $("motion-canvas").hidden = true;
    stage.append(el("div", {class: "seq-canvas", id: "seq-canvas"}));
    $("motion-stagebar")?.querySelector(".panel-actions")?.prepend(motionButton("seq-add", "Add images to the sequence", "layers", () => this.add()));
    const scrub = $("motion-scrub");
    scrub.max = String(this.duration);
    scrub.oninput = (e) => { this.pause(); this.time = Number(e.target.value); this.draw(); };
    this.built = true;
    this.renderInspector();
    this.fit();
    this.draw();
  }

  caption() {
    motion.stageHeader?.rename(`Composite · ${this.layers.length} image${this.layers.length === 1 ? "" : "s"}`);
  }

  renderLayersSection() {
    const list = el("div", {class: "motion-layers", id: "seq-layers"});
    for (const l of [...this.layers].reverse()) {
      const row = el("div", {class: "motion-layer", title: l.name},
        el("img", {src: convertFileSrc(l.file), alt: ""}),
        el("button", {class: "motion-layer-name", title: `Select ${l.name}`, onclick: () => { this.selected = l.id; this.renderInspector(); this.draw(); }}, l.name),
        motionButton(`seq-hide-${l.id}`, `Remove ${l.name}`, "minus", () => this.remove(l.id)));
      if (l.id === this.selected) row.classList.add("selected");
      list.append(row);
    }
    if (!this.layers.length) list.append(el("p", {class: "muted"}, "Add images to compose them in time."));
    this.caption();
    return list;
  }

  renderSelectedLayerSection(layer: any) {
    if (!layer) return null;
    return motionSection(layer.name, [
      motionRange(`seq-scale-${layer.id}`, "Scale", 10, 300, 1, layer.scale, "%", (v) => { layer.scale = v; this.draw(); }),
      ...["entry", "exit"].map((side) => {
        const spec = layer[side];
        return el("div", {class: "seq-side-block"},
          el("div", {class: "group-label"}, side === "entry" ? "Enter" : "Leave"),
          seqSelect("Kind", SEQ_KINDS, spec.kind, (v) => { spec.kind = v; this.draw(); this.renderInspector(); }),
          motionRange(`seq-${side}-duration-${layer.id}`, "Duration", 0.05, 3, 0.05, spec.duration, "s", (v) => { spec.duration = v; this.draw(); }),
          motionRange(`seq-${side}-delay-${layer.id}`, "Delay", 0, 20, 0.05, spec.delay, "s", (v) => { spec.delay = v; this.draw(); }),
          seqSelect("Ease", SEQ_EASES, spec.ease, (v) => { spec.ease = v; this.draw(); }));
      }),
    ], true, "motion-selected-layer");
  }

  renderInspector() {
    const box = $("motion-inspector");
    if (!box) return;
    const layer = this.layers.find((l) => l.id === this.selected);
    box.replaceChildren(
      motionSection("Canvas", [
        seqSelect("Format", Object.keys(SEQ_PRESETS), this.preset, (value) => {
          this.preset = value;
          this.canvas = SEQ_PRESETS[value].slice();
          this.layers.forEach((l) => { l.center = [this.canvas[0] / 2, this.canvas[1] / 2]; });
          this.fit(); this.renderInspector(); this.draw();
        }),
        seqField("Background", el("input", {type: "color", value: this.background, "aria-label": "Background colour",
          oninput: (e) => { this.background = e.target.value; this.fit(); }})),
        motionRange("seq-seconds", "Seconds", 1, 20, 0.5, this.duration, "s", (v) => {
          this.duration = v;
          const scrub = $("motion-scrub");
          if (scrub) scrub.max = String(v);
        }),
      ], true, "motion-canvas-settings"),
      motionSection("Layers", this.renderLayersSection(), true, "motion-layers"),
      this.renderSelectedLayerSection(layer),
      motionSection("Export & integrations", [
        el("button", {class: "start", id: "seq-export", title: "Render the sequence with EffectCraft", onclick: () => this.exportSequence()}, "Render"),
        el("div", {class: "why", id: "seq-status"}, this.busy ? "Rendering…" : this.result?.ok ? "Rendered" : ""),
        this.result?.ok ? fileButton({path: this.result.file, name: this.result.file.split("/").pop()}) : null,
      ], true, "motion-export-section"),
    );
  }

  fit() {
    const canvas = $("seq-canvas");
    if (!canvas) return;
    canvas.style.setProperty("--seq-background", this.background || "transparent");
  }

  draw() {
    const canvas = $("seq-canvas");
    if (!canvas) return;
    const [w, h] = this.canvas;
    for (const layer of this.layers) {
      let node = canvas.querySelector(`[data-layer="${layer.id}"]`);
      if (!node) {
        node = el("img", {"data-layer": layer.id, alt: layer.name, draggable: "false"});
        node.addEventListener("pointerdown", (e) => this.drag(e, layer));
        canvas.append(node);
      }
      if (node.getAttribute("src") !== convertFileSrc(layer.file)) node.src = convertFileSrc(layer.file);
      const off = this.offsets(layer, this.time);
      const scale = Math.max(0, (layer.scale + off.scale[0]) / 100);
      node.style.setProperty("--seq-x", `${((layer.center[0] + off.position[0]) / w) * 100}%`);
      node.style.setProperty("--seq-y", `${((layer.center[1] + off.position[1]) / h) * 100}%`);
      node.style.setProperty("--seq-width", `${((layer.natural[0] * scale) / w) * 100}%`);
      node.style.setProperty("--seq-opacity", String(Math.max(0, Math.min(1, (100 + off.opacity) / 100))));
      node.style.setProperty("--seq-rotation", `${off.rotation}deg`);
      node.classList.toggle("selected", layer.id === this.selected);
    }
    for (const node of [...canvas.children]) if (!this.layers.some((l) => l.id === node.dataset.layer)) node.remove();
    canvas.classList.toggle("grid", motion.grid);
    const clock = $("motion-time");
    if (clock) clock.textContent = `${Math.floor(this.time / 60)}:${(this.time % 60).toFixed(3).padStart(6, "0")}`;
    const time = $("motion-scrub");
    if (time && document.activeElement !== time) time.value = String(Math.min(this.time, this.duration));
    this.caption();
  }

  drag(event: PointerEvent, layer: any) {
    event.preventDefault();
    this.selected = layer.id;
    this.renderInspector();
    const canvas = $("seq-canvas");
    const rect = canvas.getBoundingClientRect();
    const [w, h] = this.canvas;
    const move = (e: PointerEvent) => {
      layer.center = [
        Math.max(0, Math.min(w, ((e.clientX - rect.left) / rect.width) * w)),
        Math.max(0, Math.min(h, ((e.clientY - rect.top) / rect.height) * h)),
      ];
      this.draw();
    };
    const up = () => { window.removeEventListener("pointermove", move); window.removeEventListener("pointerup", up); };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  }

  play() {
    this.playing = true;
    let last = performance.now();
    const step = (now: number) => {
      if (!this.playing) return;
      this.time += (now - last) / 1000;
      last = now;
      if (this.time > this.duration) this.time = 0;
      this.draw();
      this.tick = requestAnimationFrame(step);
    };
    this.tick = requestAnimationFrame(step);
  }

  pause() {
    this.playing = false;
    cancelAnimationFrame(this.tick);
  }

  async add() {
    const files = await dialog.open({title: "Add images", multiple: true, filters: [{name: "Images", extensions: ["png", "jpg", "jpeg", "webp"]}]});
    const list = Array.isArray(files) ? files : files ? [files] : [];
    for (const file of list) await this.addFile(file);
  }

  async addFile(file: string) {
    const natural = await new Promise((resolve) => {
      const img = new Image();
      img.onload = () => resolve([img.naturalWidth, img.naturalHeight]);
      img.onerror = () => resolve([0, 0]);
      img.src = convertFileSrc(file);
    });
    const id = `layer${this.layers.length + 1}`;
    this.layers.push({
      id, file, name: file.split("/").pop(), natural,
      center: [this.canvas[0] / 2, this.canvas[1] / 2], scale: 100,
      entry: {kind: "fade", duration: 0.6, delay: 0, ease: "smooth"},
      exit: {kind: "fade", duration: 0.5, delay: Math.max(0, this.duration - 0.5), ease: "smooth"},
    });
    this.selected = id;
    this.renderInspector();
    this.draw();
  }

  remove(id: string) {
    this.layers = this.layers.filter((l) => l.id !== id);
    if (this.selected === id) this.selected = this.layers.at(-1)?.id ?? null;
    this.renderInspector();
    this.draw();
  }

  async exportSequence() {
    if (this.busy) return;
    if (!this.layers.length) { toast("Add an image first"); return; }
    this.pause();
    this.busy = true;
    this.renderInspector();
    try {
      const result = await motionCommand("sequence.render", {
        name: "sequence", canvas: this.canvas, fps: this.fps, duration: this.duration, background: this.background,
        layers: this.layers.map((l) => ({id: l.id, file: l.file, center: l.center, scale: l.scale, entry: l.entry, exit: l.exit})),
        stills: 4, video: true,
      });
      const out = result.export?.outputs?.[0];
      this.result = {ok: Boolean(result.ok && out?.file), file: out?.file || "", validation: Boolean(result.export?.validation_ok)};
      toast(this.result.ok ? "Sequence rendered" : "Render returned no track");
    } catch (error) {
      this.result = {ok: false, file: "", error: String(error)};
      toast(String(error));
    } finally {
      this.busy = false;
      this.renderInspector();
    }
  }
}

const seq = new SequenceViewController();
function seqSideOffset(spec: any, entering: boolean, t: number, rest: any) { return seq.sideOffset(spec, entering, t, rest); }
function seqOffsets(layer: any, t: number) { return seq.offsets(layer, t); }
function sequenceAttach() { seq.attach(); }
function seqCaption() { seq.caption(); }
function seqRenderInspector() { seq.renderInspector(); }
function seqFit() { seq.fit(); }
function seqDraw() { seq.draw(); }
function seqDrag(event: PointerEvent, layer: any) { seq.drag(event, layer); }
function seqPlay() { seq.play(); }
function seqPause() { seq.pause(); }
async function seqAdd() { return seq.add(); }
async function seqAddFile(file: string) { return seq.addFile(file); }
function seqRemove(id: string) { seq.remove(id); }
async function seqExport() { return seq.exportSequence(); }
