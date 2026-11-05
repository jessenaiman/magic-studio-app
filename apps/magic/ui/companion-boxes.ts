// The shared UI-box contract.
//
// Every control in the app belongs to a reusable box. A box has a stable id, the
// project and context it belongs to, and a state that describes what the app
// knows about it: idle, working, ready, needs-attention or failed. Boxes also
// carry the scent of the work in them -- reason, priority, the correlated
// request and job, and how a failure was resolved.
//
// Two rules matter more than the shape:
//
//  1. FACTS COME FROM EVENTS. A box only turns "working", "ready" or "failed"
//     when an actual command, job or render event says so. The readiness
//     `data-state` the page already renders may inform a prerequisite status,
//     but an empty box is NOT an error and never becomes "failed".
//  2. THE SCENT IS NOT DOM-ONLY. The same observation is published to the
//     bounded feedback snapshot the TUI already reads, so the app light, the
//     pet and the TUI all react to one semantic state instead of three guesses.
//     Frontend observation is marked unavailable or stale when the app is
//     closed; nothing is ever invented.
const BOX_STATES = ["idle", "working", "ready", "needs-attention", "failed"];

// One number per state: the only thing a control has to reveal, and the only thing a pet reads.
//   0 none · 1 green in use · 2 yellow attention · 3 red problem
// Working is work in flight, so it is green and busy; yellow is only "needs you".
const SIGNAL = { idle: 0, ready: 1, working: 1, "needs-attention": 2, failed: 3 };
const SIGNAL_LABEL = { 0: "idle", 1: "in use", 2: "attention", 3: "problem" };

// The rail or box a control belongs to, if any.
function controlOwner(node: any): string | null {
  return node.closest('[data-companion-rail]')?.getAttribute('data-companion-rail') || node.closest('[data-box-id]')?.getAttribute('data-box-id');
}

function controlVisible(node: any, rect: DOMRect, computed: CSSStyleDeclaration): boolean {
  return rect.width > 0 && rect.height > 0 && computed.visibility !== 'hidden' && !node.closest('[hidden],[aria-hidden=true]');
}

// What an agent may do to a control: set or key an editable one, click a clickable one.
function controlActions(editable: boolean, clickable: boolean): string[] {
  if (editable) return ['ui.set', 'ui.key'];
  return clickable ? ['ui.click'] : [];
}

const INSPECTED_STYLES = ['display', 'visibility', 'color', 'backgroundColor', 'fontSize', 'fontWeight', 'overflow', 'position'];

// The value an inspector reports for a control: checked state, a number's range, or a select's options.
function controlData(node: any, kind: string): Record<string, any> | null {
  if (node.type === 'checkbox' || node.type === 'radio') return { checked: !!node.checked };
  if (node.matches('input[type=number],input[type=range]')) return { value: node.value, min: node.min, max: node.max, step: node.step };
  if (kind === 'select') return { value: node.value, options: [...node.options].map((o: any) => ({ value: o.value, label: o.label, disabled: o.disabled })) };
  return null;
}

// The element's own text nodes (not its children's), trimmed and capped.
function ownText(node: any): string {
  return [...(node.childNodes || [])].filter((n: any) => n.nodeType === 3).map((n: any) => n.textContent.trim()).filter(Boolean).join(' ').slice(0, 160);
}

function canonicalBoxId(id: any): string {
  return window.uiConfig?.elements?.[id]?.target || id;
}

function currentProjectPath(): string | null {
  return typeof state === "undefined" ? null : state.project ?? null;
}

function extractJobId(value: any): string | null {
  return typeof value === "string" ? value : value?.id || null;
}

function boundedSet(map: Map<any, any> | Set<any>, key: any, value?: any) {
  if (map instanceof Map) map.set(key, value);
  else map.add(key);
  while (map.size > 64) map.delete(map.keys().next().value);
}

function createEmptyBox(id: string, opts: any) {
  return {
    id,
    project: opts.project ?? null,
    context: opts.context ?? null,
    priority: opts.priority ?? 0,
    title: opts.title ?? null,
    label: opts.label ?? opts.title ?? null,
    look: opts.look ?? null,
    density: opts.density ?? null,
    open: false,
    busy: false,
    dirty: false,
    state: "idle",
    reason: null,
    request: null,
    job: null,
    resolution: null,
    source: null,
    observed_at: null,
    receipt: null,
  };
}

const OPTIONAL_BOX_FIELDS = ["context", "title", "label", "look", "density"] as const;

function applyBoxMetadata(box: any, opts: any) {
  if (opts.project != null && box.project !== opts.project) {
    Object.assign(box, { project: opts.project, context: null, resolution: null, state: "idle", busy: false, reason: null, request: null, job: null, source: null, attention: null, receipt: null, observed_at: null });
  }
  if (opts.priority) box.priority = opts.priority;
  for (const key of OPTIONAL_BOX_FIELDS) {
    if (opts[key] != null) box[key] = opts[key];
  }
}

const FACT_FIELDS = ["priority", "request", "job", "resolution"] as const;

function applyFactFields(box: any, payload: any) {
  if (payload.state && BOX_STATES.includes(payload.state)) box.state = payload.state;
  if (payload.reason != null) box.reason = String(payload.reason).slice(0, 180);
  for (const key of FACT_FIELDS) {
    if (payload[key] != null) box[key] = payload[key];
  }
}

function resolveEventOutcome(ev: any, result: any) {
  const failed = ev.phase === "error" || result.ok === false || result.state === "failed";
  const working = ev.phase === "start" || ["submitted", "queued", "running", "cancelling"].includes(result.state);
  const state = failed ? "failed" : working ? "working" : result.state === "cancelled" ? "idle" : "ready";
  const reason = failed ? String(ev.error || result.error || "Operation failed") : working ? "Working" : "Command completed";
  return { state, reason };
}

function paintIndicator(node: any, value: number) {
  node.dataset.signal = String(value);
  node.textContent = value === 0 ? (window.uiConfig?.signals?.[0]?.icon || "💤") : "";
  node.setAttribute("aria-label", SIGNAL_LABEL[value]);
}

function inspectNodeAttributes(node: any, entry: any, editable: boolean) {
  return {
    enabled: !node.disabled,
    selected: node.getAttribute('aria-selected') || node.getAttribute('aria-pressed') || null,
    label: node.getAttribute('aria-label') || node.title || entry?.label || null,
    text: editable || node.closest('#agent-log,#agent-messages') ? null : ownText(node),
    commands: (node.dataset.commands || '').split(/\s+/).filter(Boolean),
  };
}

function inspectNodeIdentity(node: any, record: any, parent: any, configId: any, entry: any, owner: any) {
  const kind = node.tagName.toLowerCase();
  return {
    index: record.index,
    id: node.id || null,
    target: node.id || `@${record.index}`,
    parent_index: parent?.index ?? null,
    parent_id: node.parentElement?.id || null,
    tag: kind,
    component: entry?.component || kind,
    config: entry ? configId : null,
    rail: entry?.rail || null,
    container: owner ? canonicalBoxId(owner) : null,
  };
}

class CompanionBoxes {
  boxes = new Map<string, any>();
  listeners = new Set<() => void>();
  jobs = new Map<string, any>();
  quiet = false;
  changeSeq = 0;
  panelRevision = 0;
  panelSignature = "";
  contractIssues = new Map<string, any[]>();
  contractSignature = "";
  elementRecords = new WeakMap<Element, { index: number }>();
  instance = `${Date.now()}-${Math.random().toString(36).slice(2)}`;
  nextIndex = 0;
  componentRevision = 0;
  componentSignature = "";
  states = BOX_STATES;

  constructor() {
    this.registerElement = this.registerElement.bind(this);
    this.inspect = this.inspect.bind(this);
    this.capture = this.capture.bind(this);
    this.event = this.event.bind(this);
    this.attention = this.attention.bind(this);
    this.register = this.register.bind(this);
    this.fact = this.fact.bind(this);
    this.readiness = this.readiness.bind(this);
    this.recordReceipt = this.recordReceipt.bind(this);
    this.get = this.get.bind(this);
    this.snapshot = this.snapshot.bind(this);
    this.panels = this.panels.bind(this);
    this.clear = this.clear.bind(this);
    this.signal = this.signal.bind(this);
    this.dot = this.dot.bind(this);
    this.subscribe = this.subscribe.bind(this);
  }

  registerElement(node: any) {
    if (!node?.tagName || node.closest?.('script,style,head')) return null;
    if (node.namespaceURI === 'http://www.w3.org/2000/svg' && node.tagName.toLowerCase() !== 'svg') return null;
    let record = this.elementRecords.get(node);
    if (!record) {
      record = { index: ++this.nextIndex };
      this.elementRecords.set(node, record);
      node.setAttribute('data-ui-index', String(record.index));
    }
    return record;
  }

  inspectNode(node: any) {
    const record = this.registerElement(node)!;
    const parent = this.registerElement(node.parentElement);
    const rect = node.getBoundingClientRect(), computed = getComputedStyle(node);
    const configId = node.dataset.uiConfig || node.id, entry = window.uiConfig?.elements?.[configId];
    const owner = controlOwner(node), live = owner ? this.get(owner) : null;
    const kind = node.tagName.toLowerCase();
    const editable = node.matches('input,select,textarea,[contenteditable=true]');
    const clickable = node.matches('button,a[href],[role=button],[role=tab],.chip,.proj,.object,.lyric');
    return {
      ...inspectNodeIdentity(node, record, parent, configId, entry, owner),
      rect: [rect.x, rect.y, rect.width, rect.height], visible: controlVisible(node, rect, computed),
      signal: live ? this.boxSignal(live) : null, busy: live ? live.state === "working" : null,
      data: controlData(node, kind),
      style: Object.fromEntries(INSPECTED_STYLES.map(key => [key, computed[key]])),
      actions: controlActions(editable, clickable),
      ...inspectNodeAttributes(node, entry, editable),
    };
  }

  inspect(opts: any = {}) {
    const root = opts.root === undefined ? document.body : opts.root;
    const expected_instance = opts.expected_instance ?? null;
    const expected_revision = opts.expected_revision ?? null;
    if (!root) throw new Error("Unknown component region");
    if (expected_instance && expected_instance !== this.instance) throw new Error("Studio component instance changed; inspect again");
    const items = [root, ...root.querySelectorAll('*')].filter(n => this.registerElement(n)).map(n => this.inspectNode(n));
    const signature = JSON.stringify([currentProjectPath(), items]);
    if (signature !== this.componentSignature) { this.componentSignature = signature; this.componentRevision++; }
    if (expected_revision !== null && Number(expected_revision) !== this.componentRevision) throw new Error("Component snapshot changed; restart pagination");
    const offset = Math.max(0, Math.floor(Number(opts.offset) || 0));
    const limit = Math.min(64, Math.max(1, Math.floor(Number(opts.limit) || 64)));
    return { schema_version: 1, instance: this.instance, revision: this.componentRevision, available: true, project: currentProjectPath(),
      total: items.length, offset, items: items.slice(offset, offset + limit), next_offset: offset + limit < items.length ? offset + limit : null };
  }

  notify() {
    if (this.quiet) return;
    this.quiet = true;
    queueMicrotask(() => {
      this.quiet = false;
      for (const node of document.querySelectorAll('[data-panel-indicator]')) {
        const box = this.get(node.getAttribute('data-panel-indicator'));
        if (box) paintIndicator(node, this.boxSignal(box));
      }
      for (const fn of this.listeners) {
        try { fn(); } catch (e) { console.warn(String(e)); }
      }
    });
  }

  register(rawId: any, opts: any = {}) {
    if (!rawId) return null;
    const id = canonicalBoxId(rawId);
    const entry = window.uiConfig?.elements?.[id];
    const resolved = {
      ...opts,
      priority: entry?.priority ?? opts.priority ?? 0,
      label: rawId !== id ? (entry?.label ?? opts.label) : opts.label,
      look: rawId !== id ? (entry?.look ?? opts.look) : opts.look,
      density: rawId !== id ? (entry?.density ?? opts.density) : opts.density,
    };
    let box = this.boxes.get(id);
    if (!box) {
      box = createEmptyBox(id, resolved);
      this.boxes.set(id, box);
    } else {
      applyBoxMetadata(box, resolved);
    }
    return box;
  }

  fact(payload: any = {}) {
    const box = this.register(payload.id, { project: payload.project ?? null, context: payload.context ?? null });
    if (!box) return null;
    applyFactFields(box, payload);
    box.source = payload.source ?? "engine";
    box.observed_at = payload.observed_at || Date.now();
    box.seq = ++this.changeSeq;
    this.notify();
    return box;
  }

  readiness(id: any, opts: any = {}) {
    const state = opts.state ?? null, reason = opts.reason ?? null, project = opts.project ?? null;
    const box = this.register(id, { project: project != null ? project : currentProjectPath() });
    if (!box) return null;
    const next = state === "needs" ? "needs-attention" : BOX_STATES.includes(state) ? state : null;
    if (next && box.state !== next && box.state !== "working" && box.state !== "failed") {
      box.state = next;
      if (reason != null) box.reason = String(reason).slice(0, 180);
      box.source = "readiness";
      box.observed_at = Date.now();
      box.seq = ++this.changeSeq;
      this.notify();
    }
    return box;
  }

  recordReceipt(id: string, result: any = {}) {
    const box = this.boxes.get(canonicalBoxId(id));
    if (!box) return null;
    const dirty = result.dirty;
    box.receipt = {
      undo: result.undo ?? null,
      revision: result.revision ?? result._ui_engine_revision ?? null,
      saved: result.saved ?? (dirty === false ? true : dirty === true ? false : null),
      command: result.command ?? null,
      at: Date.now(),
    };
    box.observed_at = Date.now();
    box.seq = ++this.changeSeq;
    this.notify();
    return box.receipt;
  }

  shouldIgnoreEvent(box: any, context: any, ev: any, request: any): boolean {
    if (!box?.request || !request || box.request === request) return false;
    return Boolean(context.job || (ev.phase !== "start" && box.state === "working"));
  }

  event(ev: any) {
    if (ev.project !== state.project) return;
    const context = ev.box ? ev : this.capture(ev.command, ev.params);
    if (!context.box) return;
    const box = this.get(context.box), request = context.job ? context.request : (ev.request || context.request);
    if (this.shouldIgnoreEvent(box, context, ev, request)) return;
    const result = ev.result || {};
    const job = extractJobId(result.job) || extractJobId(result.job_id) || context.job;
    if (job) boundedSet(this.jobs, `${ev.project}:${job}`, { box: context.box, request });
    const outcome = resolveEventOutcome(ev, result);
    this.fact({ id: context.box, project: ev.project, job, state: outcome.state, request, reason: outcome.reason, source: "command result" });
    if (ev.phase === "success") this.recordReceipt(context.box, { ...result, command: ev.command });
  }

  findCommandElement(command: string, controlId?: string) {
    if (controlId) {
      const byId = document.getElementById(controlId);
      if (byId) return byId;
    }
    return [...document.querySelectorAll("[data-commands]")].find(n => (n.getAttribute("data-commands") || "").split(/\s+/).includes(command)) || null;
  }

  capture(command: string, params: any = {}) {
    const action = typeof acFeedback === "undefined" ? null : acFeedback.action;
    const job = extractJobId(params.job) || extractJobId(params.job_id);
    const owner = job ? this.jobs.get(`${state.project}:${job}`) : null;
    const node = this.findCommandElement(command, params?._control || action?.control);
    const box = owner?.box || action?.box || (node ? controlOwner(node) : null) || null;
    return { command, params, box, job, request: owner?.request || params?._request_id || action?.request_id || null, project: state.project, view: state.view };
  }

  attention(id: any, reason: any) {
    if (!this.boxes.has(canonicalBoxId(id))) throw new Error("Unknown panel: " + id);
    if (reason !== null && (typeof reason !== "string" || !reason.trim())) throw new Error("Attention needs a reason");
    const box = this.register(id, { project: currentProjectPath() });
    box.project = currentProjectPath();
    box.attention = reason === null ? null : reason.trim().slice(0, 180);
    box.seq = ++this.changeSeq;
    box.observed_at = Date.now();
    this.notify();
  }

  boxSignal(box: any): number {
    return Math.max(SIGNAL[box.state] ?? 0, box.attention ? 2 : 0, this.contractIssues.has(box.id) ? 3 : 0);
  }

  get(id: any) {
    return this.boxes.get(canonicalBoxId(id)) || null;
  }

  snapshot(opts: any = {}) {
    const project = opts.project ?? null, limit = opts.limit ?? 64;
    const rows = [];
    for (const box of this.boxes.values()) {
      if (project && box.project && box.project !== project) continue;
      rows.push({
        id: box.id, label: box.label ?? null, look: box.look ?? null, busy: box.state === "working",
        project: box.project, context: box.context, state: box.state, signal: this.boxSignal(box),
        attention: box.attention ?? null, seq: box.seq ?? 0,
        reason: this.contractIssues.has(box.id) ? `UI contract: ${this.contractIssues.get(box.id)!.map(issue => issue.id).join(", ")}` : box.attention || box.reason,
        priority: box.priority, request: box.request, job: box.job, resolution: box.resolution,
        source: box.source, observed_at: box.observed_at, receipt: box.receipt,
      });
    }
    rows.sort((a, b) => (b.priority || 0) - (a.priority || 0) || a.id.localeCompare(b.id));
    return rows.slice(0, limit);
  }

  syncContractIssues(unconfigured: any[]) {
    const nextSignature = JSON.stringify(unconfigured);
    if (nextSignature === this.contractSignature) return;
    this.contractSignature = nextSignature;
    this.contractIssues = new Map();
    for (const issue of unconfigured) {
      const target = canonicalBoxId(issue.container || window.uiConfig?.contract_target);
      if (!this.boxes.has(target)) continue;
      if (!this.contractIssues.has(target)) this.contractIssues.set(target, []);
      this.contractIssues.get(target)!.push(issue);
    }
    this.notify();
  }

  panels() {
    const ui = typeof window !== "undefined" ? window.magicUI : null;
    const unconfigured = ui && typeof ui.unconfigured === "function" ? ui.unconfigured() : [];
    this.syncContractIssues(unconfigured);
    const items = this.snapshot({ project: currentProjectPath(), limit: Infinity });
    const signature = JSON.stringify([items.map(({ seq, observed_at, ...item }) => item), unconfigured]);
    if (signature !== this.panelSignature) { this.panelSignature = signature; this.panelRevision++; }
    return { revision: this.panelRevision, available: true, items, unconfigured, total_items: items.length, total_unconfigured: unconfigured.length, truncated: false };
  }

  clear() {
    this.boxes.clear();
    this.jobs.clear();
    this.notify();
  }

  signal(state: string): number {
    return SIGNAL[state] ?? 0;
  }

  dot(state: string, variant = "", id: string | null = null) {
    if (id) this.show(id, state);
    const value = this.signal(state);
    const node = document.createElement("span");
    node.className = "dot";
    const canonical = id ? canonicalBoxId(id) : null;
    paintIndicator(node, canonical && this.boxes.has(canonical) ? this.boxSignal(this.boxes.get(canonical)) : value);
    if (id) node.setAttribute("data-panel-indicator", id);
    node.hidden = !!id && window.uiConfig?.elements?.[id]?.indicator === "hidden";
    if (variant) node.dataset.variant = variant;
    node.setAttribute("role", "img");
    return node;
  }

  show(id: string, state: string) {
    if (!BOX_STATES.includes(state)) return;
    const box = this.register(id);
    if (!box || box.state === state || box.state === "working" || box.state === "failed") return;
    box.state = state;
    box.source = "light";
    box.observed_at = Date.now();
    box.seq = ++this.changeSeq;
    this.notify();
  }

  subscribe(fn: () => void) {
    this.listeners.add(fn);
    return () => this.listeners.delete(fn);
  }
}

function createCompanionBoxes() {
  return new CompanionBoxes();
}

// One shared registry, created before the app renders, so every control can
// join the contract the moment it is built.
window.uiRegistry = window.companionBoxes = createCompanionBoxes();

window.uiRegistry.subscribe(() => window.acFeedback?.report());
