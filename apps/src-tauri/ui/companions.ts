// Companion pets. They follow one thing: the signal a control already carries.
//
//   0 none · 1 green in use · 2 yellow attention · 3 red problem
//
// Sproket goes to the worst red/yellow and poses alert or work. Gaffer follows the most recently
// changed signal and wanders there. No command is inspected, no job is correlated and no per-control
// knowledge lives here: a box reports a state, the state maps to a number, the number picks a pose.
class CompanionPetsController {
  boxes: any;
  enabled: boolean;
  frames: any;
  media: any;
  nodes: Record<string, any> = {};
  pets: Record<string, any> = {};
  project: string | null = null;
  rails: any;
  settleTimer: any = null;
  sprites: any;
  stage: any = null;
  travelling: Record<string, any> = {};

  constructor(frames: any, options: any = {}) {
    this.frames = frames;
    this.sprites = options.sprites || null;
    this.rails = options.rails || null;
    this.boxes = options.boxes || window.companionBoxes || null;
    this.media = globalThis.matchMedia?.("(prefers-reduced-motion: reduce)") || {matches: false, addEventListener() {}};
    this.enabled = localStorage.getItem("magic.companions") === "true";
    document.body.dataset.companionsEnabled = String(this.enabled);

    for (const name of ["sproket", "gaffer"]) {
      const node = el("pre", {id: `companion-${name}`, "aria-hidden": "true", inert: ""});
      this.nodes[name] = node;
      this.pets[name] = {pose: "rest", box: null, slot: null, signal: 0, timer: null, step: 0, lastBurstAt: 0};
    }

    document.addEventListener?.("visibilitychange", this.sync);
    this.media.addEventListener?.("change", this.sync);
    const reposition = () => {
      this.rails?.invalidate();
      for (const name of Object.keys(this.nodes)) this.place(name);
    };
    globalThis.addEventListener?.("resize", reposition);
    globalThis.addEventListener?.("scroll", reposition, true);

    // Change-driven: the page reports box changes; nothing here polls or scans per frame.
    this.boxes?.subscribe?.(() => this.react());
    this.sync();
  }

  useSprite(name: string) {
    return Boolean(this.sprites && this.sprites.has(name));
  }

  spriteElement(name: string) {
    const img = this.sprites.image(name, this.pets[name].pose || "rest");
    if (!img) return null;
    if (!this.nodes[name].sprite) {
      const holder = el("span", {id: `companion-${name}-sprite`});
      this.nodes[name].sprite = holder;
      this.nodes[name].replaceChildren(holder);
    }
    const holder = this.nodes[name].sprite;
    if (holder.firstChild !== img) {
      holder.replaceChildren(img);
      holder.hidden = false;
    }
    return holder;
  }

  frame() {
    for (const name of Object.keys(this.nodes)) {
      if (this.useSprite(name)) { this.spriteElement(name); continue; }
      const bank = this.frames?.frames?.[name];
      if (!bank?.length) continue;
      const text = bank[this.pets[name].step % bank.length];
      if (this.nodes[name].textContent !== text) this.nodes[name].textContent = text;
    }
  }

  slotFor(name: string, boxId: string | null) {
    const config = window.uiConfig?.companions?.[name];
    if (!this.rails || !config) return null;
    return this.rails.resolve({name, role: config.role, target: boxId || config.home});
  }

  travel(name: string, slot: any) {
    const node = this.nodes[name];
    if (!slot || !this.rails) return;
    const rect = this.rails.measure(slot.slotId);
    if (!rect) return;
    this.pets[name].slot = slot.slotId;
    if (!this.stage) {
      this.stage = el("div", {id: "companion-stage", "aria-hidden": "true", inert: ""});
      document.body.append(this.stage);
    }
    if (node.parentElement !== this.stage) this.stage.append(node);
    node.classList.add("companion-traveling");
    const peers = Object.keys(this.pets).filter(p => this.pets[p].slot === slot.slotId);
    const offset = rect.width * peers.indexOf(name) / peers.length;
    node.style.setProperty("--companion-position", `translate3d(${Math.round(rect.x + offset)}px, ${Math.round(rect.y)}px, 0)`);
    clearTimeout(this.travelling[name]);
    this.travelling[name] = setTimeout(() => {
      delete this.travelling[name];
      node.classList.remove("companion-traveling");
    }, 900);
  }

  place(name: string) {
    const pet = this.pets[name];
    if (!this.enabled || !this.project || document.hidden) {
      this.nodes[name].hidden = true;
      return;
    }
    const slot = this.slotFor(name, pet.box);
    const rect = slot && this.rails.measure(slot.slotId);
    this.nodes[name].hidden = !rect;
    pet.placement_error = rect ? null : "No configured visible rail for this component";
    if (!rect) { pet.slot = null; return; }
    this.travel(name, slot);
  }

  burst(name: string) {
    if (!this.enabled || document.hidden || this.media.matches || !this.useSprite(name)) return;
    const pet = this.pets[name];
    const now = Date.now();
    if (now - (pet.lastBurstAt || 0) < 1500) return;
    pet.lastBurstAt = now;
    const poses = ["work", "alert", pet.pose];
    clearTimeout(pet.timer);
    pet.step = 0;
    const tick = () => {
      this.pets[name].pose = poses[pet.step];
      this.frame();
      if (++pet.step >= poses.length) { pet.timer = null; return; }
      pet.timer = setTimeout(tick, 260);
    };
    pet.timer = setTimeout(tick, 260);
  }

  selectTargets(rows: any[]) {
    const newestFirst = (a: any, b: any) => (b.seq ?? 0) - (a.seq ?? 0) || (b.observed_at ?? 0) - (a.observed_at ?? 0);
    const worst = rows.filter(row => row.signal >= 2).sort((a, b) => b.signal - a.signal || Number(!!b.attention) - Number(!!a.attention) || (b.priority || 0) - (a.priority || 0) || newestFirst(a, b))[0] || null;
    const recent = rows.filter(row => row.state === "working" || (row.source !== "readiness" && row.source !== "light" && Date.now() - row.observed_at < 8000));
    const newest = [...recent].sort(newestFirst)[0] || worst;
    return {
      newest,
      targets: {
        sproket: {box: worst?.id || null, signal: worst?.signal || 0, pose: worst ? (worst.signal === 3 ? "alert" : "work") : "rest"},
        gaffer: {box: newest?.id || null, signal: newest?.signal || 0, pose: newest ? (newest.state === "working" || newest.signal >= 2 ? "work" : "rest") : "asleep"},
      },
    };
  }

  applyTarget(name: string, target: any) {
    const pet = this.pets[name];
    const news = pet.box !== target.box || pet.pose !== target.pose;
    if (news && target.signal < 2) { clearTimeout(pet.timer); pet.timer = null; }
    Object.assign(pet, target);
    if (news && target.signal >= 2) this.burst(name);
  }

  react = () => {
    if (!this.boxes) return;
    if (!this.enabled) {
      for (const name of Object.keys(this.nodes)) this.nodes[name].hidden = true;
      return;
    }
    this.rails?.discover();
    const rows = this.boxes.panels().items.filter(row => row.signal > 0);
    const {newest, targets} = this.selectTargets(rows);
    clearTimeout(this.settleTimer);
    if (newest?.state === "ready") {
      this.settleTimer = setTimeout(this.react, Math.max(1, 8000 - (Date.now() - newest.observed_at)));
    }
    for (const name of Object.keys(targets)) this.applyTarget(name, targets[name]);
    for (const name of Object.keys(this.nodes)) this.place(name);
    this.frame();
    if (typeof window.magicCompanionPaint === "function") window.magicCompanionPaint();
  };

  reset() {
    clearTimeout(this.settleTimer);
    for (const name of Object.keys(this.pets)) clearTimeout(this.pets[name].timer);
    for (const name of Object.keys(this.pets)) {
      this.pets[name] = {pose: "rest", box: null, slot: null, signal: 0, timer: null, step: 0, lastBurstAt: 0};
    }
    for (const name of Object.keys(this.nodes)) {
      this.nodes[name].hidden = true;
      this.nodes[name].style.removeProperty("--companion-position");
      delete this.nodes[name].sprite;
      this.nodes[name].replaceChildren();
    }
  }

  sync = () => {
    if (this.project !== state.project) {
      this.project = state.project;
      this.reset();
    }
    if (!this.enabled || this.media.matches || document.hidden) {
      clearTimeout(this.settleTimer);
      for (const pet of Object.values(this.pets)) { clearTimeout(pet.timer); pet.timer = null; }
      for (const name of Object.keys(this.travelling)) {
        clearTimeout(this.travelling[name]);
        delete this.travelling[name];
        this.nodes[name].classList.remove("companion-traveling");
      }
    }
    if (!this.enabled) {
      for (const name of Object.keys(this.nodes)) this.nodes[name].hidden = true;
      return;
    }
    this.react();
  };

  setEnabled = (value: boolean) => {
    this.enabled = !!value;
    document.body.dataset.companionsEnabled = String(this.enabled);
    localStorage.setItem("magic.companions", String(this.enabled));
    this.sync();
  };

  snapshot = () => {
    return {
      enabled: this.enabled,
      project: this.project,
      reducedMotion: this.media.matches,
      hidden: document.hidden,
      sproket: {...this.pets.sproket},
      gaffer: {...this.pets.gaffer},
      boxes: this.boxes ? this.boxes.panels().items : [],
    };
  };
}

function createCompanions(frames: any, options: any = {}) {
  return new CompanionPetsController(frames, options);
}

async function initCompanions() {
  const response = await fetch("companions.frames.json");
  const frames = response.ok ? await response.json() : {fps: 2, burst_ms: 1500, frames: {}};
  const sprites = typeof createCompanionSprites === "function" ? createCompanionSprites() : null;
  if (sprites) await sprites.load();
  const boxes = typeof createCompanionBoxes === "function" ? window.companionBoxes : null;
  const rails = typeof createCompanionRails === "function" ? createCompanionRails() : null;
  const controller = createCompanions(frames, {sprites, boxes, rails});
  window.magicCompanions = controller;
  window.magicCompanionBoxes = boxes;

  const list = document.getElementById("companion-state");
  const paint = () => {
    if (!list || !boxes) return;
    const rows = boxes.panels().items.filter(row => row.signal > 0);
    list.replaceChildren(...rows.slice(0, 6).map(row => {
      return el("li", {}, `${row.id}: signal ${row.signal}${row.reason ? ` — ${row.reason}` : ""}`);
    }));
  };
  paint();

  const setting = document.getElementById("ambient-companions") as HTMLInputElement | null;
  if (setting) {
    setting.checked = controller.snapshot().enabled;
    setting.defaultChecked = setting.checked;
    setting.addEventListener("change", () => {
      controller.setEnabled(setting.checked);
      setting.defaultChecked = setting.checked;
      paint();
    });
  }
  window.magicCompanionPaint = paint;
}

if (document.getElementById("ambient-companions")) initCompanions().catch(error => console.warn(String(error)));
