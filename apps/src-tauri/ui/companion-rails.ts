// Resolve configured panel slots from their actual layout. Missing targets never fall back to unrelated panels.
function railAccepts(rail: any, name: string, role: any): boolean {
  if (rail.acceptAll) {
    if (!rail.roles.length) return true;
    return !role || rail.roles.includes(role);
  }
  return rail.accept.includes(name);
}

class CompanionRails {
  root: ParentNode;
  rails = new Map<string, any>();
  slots = new Map<string, any>();
  measured = new Map<string, any>();
  generation = 0;

  constructor(opts: any = {}) {
    this.root = opts.root || document;
    this.discover = this.discover.bind(this);
    this.invalidate = this.invalidate.bind(this);
    this.measure = this.measure.bind(this);
    this.railVisible = this.railVisible.bind(this);
    this.resolve = this.resolve.bind(this);
    this.eligibleSlots = this.eligibleSlots.bind(this);
  }

  get railCount() { return this.rails.size; }
  get slotCount() { return this.slots.size; }

  discoverRails() {
    for (const element of this.root.querySelectorAll("[data-companion-rail]")) {
      const id = element.getAttribute("data-companion-rail");
      const config = id ? window.uiConfig?.rails?.[id] : null;
      if (!id || !config) continue;
      const accept = config.accept || "yes";
      this.rails.set(id, {
        id,
        element,
        acceptAll: accept === "yes",
        accept: accept === "yes" ? [] : accept.split(/[\s,]+/).filter(Boolean),
        roles: config.roles || [],
        priority: Number(config.priority) || 0,
        slots: [],
      });
    }
  }

  discoverSlots() {
    for (const element of this.root.querySelectorAll("[data-companion-slot]")) {
      const id = element.getAttribute("data-companion-slot");
      const railId = element.closest("[data-companion-rail]")?.getAttribute("data-companion-rail") || null;
      const config = id ? window.uiConfig?.slots?.[id] : null;
      if (!id || !railId || !this.rails.has(railId) || !config) continue;
      const slot = { id, railId, element, target: config.target || id, priority: Number(config.priority) || 0, order: this.slots.size };
      this.slots.set(id, slot);
      this.rails.get(railId).slots.push(slot);
    }
  }

  discover() {
    this.rails.clear();
    this.slots.clear();
    this.measured.clear();
    this.generation++;
    this.discoverRails();
    this.discoverSlots();
    return this.rails;
  }

  eligibleSlots(opts: any = {}) {
    const { name, role, target = null } = opts;
    const found = [];
    for (const rail of this.rails.values()) {
      if (!railAccepts(rail, name, role)) continue;
      for (const slot of rail.slots) {
        if (target && slot.target !== target) continue;
        found.push({ rail, slot });
      }
    }
    return found;
  }

  isSlotInViewport(entry: any): boolean {
    const rect = this.measure(entry.slot.id);
    if (!this.railVisible(entry.rail.id) || !rect || rect.x < 0 || rect.y < 0) return false;
    return rect.x + rect.width <= (globalThis.innerWidth ?? Infinity) && rect.y + rect.height <= (globalThis.innerHeight ?? Infinity);
  }

  resolve(opts: any = {}) {
    const target = opts.target ?? null;
    if (!target) return null;
    const candidates = this.eligibleSlots({ name: opts.name, role: opts.role ?? null, target }).filter(entry => this.isSlotInViewport(entry));
    candidates.sort((a, b) => b.slot.priority - a.slot.priority || b.rail.priority - a.rail.priority || a.slot.order - b.slot.order);
    if (!candidates.length) return null;
    const { rail, slot } = candidates[0];
    return { railId: rail.id, slotId: slot.id, target: slot.target, element: slot.element, rail };
  }

  measure(slotId: string) {
    const slot = this.slots.get(slotId);
    if (!slot) return null;
    const cached = this.measured.get(slotId);
    if (cached && cached.generation === this.generation) return cached.rect;
    const rect = slot.element.getBoundingClientRect();
    if (![rect.left, rect.top, rect.width, rect.height].every(Number.isFinite) || rect.width <= 0 || rect.height <= 0) return null;
    const value = { x: rect.left, y: rect.top, width: rect.width, height: rect.height };
    this.measured.set(slotId, { generation: this.generation, rect: value });
    return value;
  }

  invalidate() {
    this.measured.clear();
    this.generation++;
  }

  railVisible(railId: string): boolean {
    const rail = this.rails.get(railId);
    if (!rail) return false;
    const rect = rail.element.getBoundingClientRect();
    return rect.width > 0 && rect.height > 0;
  }
}

function createCompanionRails(opts: any = {}) {
  return new CompanionRails(opts);
}
