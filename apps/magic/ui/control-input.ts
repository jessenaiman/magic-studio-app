// Synthetic input for the desktop control channel (`ui.click`, `ui.move`, `ui.drag`, `ui.scroll`,
// `ui.key`, `ui.type`). Craft apps inject egui pointer and key events; a webview cannot, so this
// dispatches DOM events at the element under a point. That reaches every handler and default action
// that does not require a *trusted* event (clicks, form changes, text entry, focus, scrolling) but,
// like any synthetic input, it cannot trigger CSS `:hover` or native drag-and-drop. Unsupported
// requests throw instead of reporting success.

export type Params = Record<string, unknown>;
export interface Surface { doc: Document; x: number; y: number }
interface Mods { ctrl: boolean; shift: boolean; alt: boolean; meta: boolean }
export interface Target { doc: Document; element: Element; x: number; y: number }

type Win = Window & typeof globalThis;
type Locator = Params | string;

const FOCUSABLE = 'a[href],button,input,select,textarea,summary,[tabindex],[contenteditable=""],[contenteditable="true"]';
const MODIFIER_NAMES: Record<string, keyof Mods> = {
  ctrl: "ctrl", control: "ctrl", cmd: "ctrl", command: "ctrl", meta: "meta", super: "meta",
  shift: "shift", alt: "alt", option: "alt",
};
const KEY_ALIASES: Record<string, string> = {
  space: " ", spacebar: " ", esc: "Escape", return: "Enter", del: "Delete", up: "ArrowUp", down: "ArrowDown",
  left: "ArrowLeft", right: "ArrowRight", ins: "Insert", pgup: "PageUp", pgdn: "PageDown",
};
const BUTTONS: Record<string, number> = { left: 0, middle: 1, right: 2 };
const MAX_TYPED_CHARACTERS = 10_000;

export function surfacesOf(root: Document = document, x = 0, y = 0): Surface[] {
  const found: Surface[] = [{ doc: root, x, y }];
  for (const frame of root.querySelectorAll<HTMLIFrameElement>("iframe")) {
    if (!frame.getClientRects().length) continue;
    // External video players keep their own origin and stay opaque.
    try { const child = frame.contentDocument; if (child) { const rect = frame.getBoundingClientRect(); found.push(...surfacesOf(child, x + rect.x, y + rect.y)); } } catch { /* cross-origin */ }
  }
  return found;
}

export function findElement(id: string): HTMLElement | null {
  for (const { doc } of surfacesOf()) { const element = doc.getElementById(id); if (element) return element; }
  return null;
}

const win = (doc: Document) => doc.defaultView as Win;
const isVisible = (element: Element) => element.getClientRects().length > 0;

function finite(params: Params, key: string, fallback?: number): number {
  const value = params[key] ?? fallback;
  if (typeof value !== "number" || !Number.isFinite(value)) throw new Error(`${key} must be a finite number`);
  return value;
}

function integer(params: Params, key: string, min: number, max: number, fallback: number): number {
  const value = params[key] ?? fallback;
  if (typeof value !== "number" || !Number.isInteger(value) || value < min || value > max) throw new Error(`${key} must be an integer from ${min} to ${max}`);
  return value;
}

export function allowKeys(params: Params, method: string, allowed: string[]) {
  const unknown = Object.keys(params).find(key => !allowed.includes(key));
  if (unknown) throw new Error(`${method}: unknown parameter \`${unknown}\`; accepted: ${allowed.join(", ")}`);
}

function modifiersOf(value: unknown, extra: string[] = []): Mods {
  const names = [...(Array.isArray(value) ? value : value === undefined ? [] : [value]), ...extra];
  const mods: Mods = { ctrl: false, shift: false, alt: false, meta: false };
  for (const name of names) {
    const mod = typeof name === "string" ? MODIFIER_NAMES[name.toLowerCase()] : undefined;
    if (!mod) throw new Error(`Unknown modifier ${JSON.stringify(name)}; use Ctrl, Shift, Alt or Meta`);
    mods[mod] = true;
  }
  return mods;
}

/** The element a locator names (`{id}` or `{x,y}` in window coordinates, or a bare id); the returned point is in the element's own document. */
export function locate(spec: Locator): Target {
  const params: Params = typeof spec === "string" ? { id: spec } : spec;
  if (typeof params.id === "string") {
    const element = findElement(params.id);
    if (!element || !isVisible(element)) throw new Error("Control is missing or hidden");
    if (element.matches(":disabled")) throw new Error("Control is disabled");
    const rect = element.getBoundingClientRect();
    return { doc: element.ownerDocument, element, x: rect.x + rect.width / 2, y: rect.y + rect.height / 2 };
  }
  let x = finite(params, "x"), y = finite(params, "y");
  let doc = document;
  let element = doc.elementFromPoint(x, y);
  for (let depth = 0; element instanceof HTMLIFrameElement && depth < 8; depth++) {
    let inner: Document | null = null;
    try { inner = element.contentDocument; } catch { /* cross-origin */ }
    if (!inner) break;
    const rect = element.getBoundingClientRect();
    x -= rect.x; y -= rect.y; doc = inner;
    const next = doc.elementFromPoint(x, y);
    if (!next) break;
    element = next;
  }
  if (!element) throw new Error(`Nothing is drawn at (${params.x}, ${params.y})`);
  return { doc, element, x, y };
}

function mouseInit(target: Target, button: number, buttons: number, mods: Mods, detail: number): MouseEventInit {
  return {
    bubbles: true, cancelable: true, composed: true, view: win(target.doc), detail,
    clientX: target.x, clientY: target.y, screenX: target.x, screenY: target.y,
    button, buttons, ctrlKey: mods.ctrl, shiftKey: mods.shift, altKey: mods.alt, metaKey: mods.meta,
  };
}

function fireMouse(target: Target, type: string, init: MouseEventInit) {
  return target.element.dispatchEvent(new (win(target.doc).MouseEvent)(type, init));
}

function firePointer(target: Target, type: string, init: MouseEventInit) {
  const Pointer = win(target.doc).PointerEvent;
  if (typeof Pointer !== "function") return true;
  return target.element.dispatchEvent(new Pointer(type, { ...init, pointerId: 1, pointerType: "mouse", isPrimary: true }));
}

/** A real press focuses the nearest focusable ancestor, or clears focus when there is none. */
function focusUnder(target: Target) {
  const focusable = target.element.closest<HTMLElement>(FOCUSABLE);
  if (focusable && !focusable.matches(":disabled")) focusable.focus();
  else (target.doc.activeElement as HTMLElement | null)?.blur?.();
}

function press(target: Target, button: number, mods: Mods, detail: number) {
  const init = mouseInit(target, button, 1 << button, mods, detail);
  firePointer(target, "pointerdown", init);
  if (fireMouse(target, "mousedown", init)) focusUnder(target);
}

function release(target: Target, button: number, mods: Mods, detail: number) {
  const init = mouseInit(target, button, 0, mods, detail);
  firePointer(target, "pointerup", init);
  fireMouse(target, "mouseup", init);
}

// Editors can replace the selected node in a pointer handler. Subsequent input
// must still bubble through the live document, as real pointer input would.
function liveTarget(target: Target): Target {
  if (target.element.isConnected) return target;
  const element = target.doc.elementFromPoint(target.x, target.y);
  if (!element) throw new Error("Drag target disappeared outside the visible document");
  return { ...target, element };
}

function describe(target: Target) {
  return { id: target.element.id || undefined, tag: target.element.tagName.toLowerCase(), x: target.x, y: target.y };
}

export function click(params: Params) {
  allowKeys(params, "ui.click", ["id", "x", "y", "button", "count", "modifiers"]);
  const buttonName = String(params.button ?? "left");
  const button = BUTTONS[buttonName];
  if (button === undefined) throw new Error("button must be left, middle or right");
  const count = integer(params, "count", 1, 10, 1);
  const mods = modifiersOf(params.modifiers);
  const target = locate(params);
  for (let n = 1; n <= count; n++) {
    press(target, button, mods, n);
    release(target, button, mods, n);
    const init = mouseInit(target, button, 0, mods, n);
    if (button === 0) fireMouse(target, "click", init);
    else if (button === 1) fireMouse(target, "auxclick", init);
    else fireMouse(target, "contextmenu", init);
    if (button === 0 && n === 2) fireMouse(target, "dblclick", init);
  }
  return { ok: true, ...describe(target), button: buttonName, count };
}

export function move(params: Params) {
  allowKeys(params, "ui.move", ["id", "x", "y", "modifiers", "count", "button"]);
  const target = locate(params);
  const init = mouseInit(target, 0, 0, modifiersOf(params.modifiers), 0);
  firePointer(target, "pointermove", init);
  fireMouse(target, "mousemove", init);
  return { ok: true, ...describe(target) };
}

export function drag(params: Params) {
  allowKeys(params, "ui.drag", ["from", "to", "steps", "modifiers", "button"]);
  if (params.from === undefined || params.to === undefined) throw new Error("ui.drag needs from and to, each {id} or {x,y}");
  const steps = Math.max(2, integer(params, "steps", 0, 1024, 12));
  const mods = modifiersOf(params.modifiers);
  const from = locate(params.from as Locator), to = locate(params.to as Locator);
  const button = BUTTONS[String(params.button ?? "left")];
  if (button === undefined) throw new Error("button must be left, middle or right");
  press(from, button, mods, 1);
  let last = from;
  for (let step = 1; step <= steps; step++) {
    const t = step / steps;
    const x = from.x + (to.x - from.x) * t, y = from.y + (to.y - from.y) * t;
    // Only the first and last points need a drawn element; in between, keep the last good target.
    last = step === steps ? to : { ...last, x, y };
    last = liveTarget(last);
    const init = mouseInit(last, button, 1 << button, mods, 0);
    firePointer(last, "pointermove", init);
    fireMouse(last, "mousemove", init);
  }
  release(liveTarget(to), button, mods, 1);
  return { ok: true, from: describe(from), to: describe(to), steps };
}

function scrollParent(element: Element): Element | null {
  for (let node: Element | null = element; node; node = node.parentElement) {
    const style = win(node.ownerDocument).getComputedStyle(node);
    const canScrollY = /(auto|scroll)/.test(style.overflowY) && node.scrollHeight > node.clientHeight;
    const canScrollX = /(auto|scroll)/.test(style.overflowX) && node.scrollWidth > node.clientWidth;
    if (canScrollX || canScrollY) return node;
  }
  return element.ownerDocument.scrollingElement;
}

export function scroll(params: Params) {
  allowKeys(params, "ui.scroll", ["id", "x", "y", "dx", "dy", "modifiers"]);
  const dx = finite(params, "dx", 0), dy = finite(params, "dy", 0);
  const target = locate(params);
  const mods = modifiersOf(params.modifiers);
  const wheel = new (win(target.doc).WheelEvent)("wheel", { ...mouseInit(target, 0, 0, mods, 0), deltaX: dx, deltaY: dy, deltaMode: 0 });
  const unhandled = target.element.dispatchEvent(wheel);
  // A synthetic wheel event never scrolls by itself.
  const container = unhandled ? scrollParent(target.element) : null;
  const before = container ? [container.scrollLeft, container.scrollTop] : [0, 0];
  container?.scrollBy(dx, dy);
  const moved = !!container && (container.scrollLeft !== before[0] || container.scrollTop !== before[1]);
  return { ok: true, ...describe(target), dx, dy, scrolled: moved, handled: !unhandled };
}

function deepActive(): { doc: Document; element: Element } {
  let doc = document;
  let element: Element | null = doc.activeElement;
  for (let depth = 0; element instanceof HTMLIFrameElement && depth < 8; depth++) {
    let inner: Document | null = null;
    try { inner = element.contentDocument; } catch { /* cross-origin */ }
    if (!inner) break;
    doc = inner; element = doc.activeElement;
  }
  return { doc, element: element ?? doc.body };
}

export function focusedElement() {
  const { element } = deepActive();
  return element === element.ownerDocument.body ? null : { id: element.id || undefined, tag: element.tagName.toLowerCase() };
}

type Editable = HTMLInputElement | HTMLTextAreaElement;
const TEXT_INPUTS = /^(text|search|url|tel|email|password|number|)$/;
function isEditable(element: Element): element is Editable {
  if (element instanceof win(element.ownerDocument).HTMLTextAreaElement) return !element.readOnly && !element.disabled;
  return element instanceof win(element.ownerDocument).HTMLInputElement
    && TEXT_INPUTS.test(element.type) && !element.readOnly && !element.disabled;
}

function editValue(element: Editable, edit: (value: string, start: number, end: number) => { value: string; caret: number }, inputType: string, data: string | null) {
  const value = element.value;
  const start = element.selectionStart ?? value.length, end = element.selectionEnd ?? value.length;
  const next = edit(value, start, end);
  const proto = element instanceof win(element.ownerDocument).HTMLTextAreaElement ? win(element.ownerDocument).HTMLTextAreaElement.prototype : win(element.ownerDocument).HTMLInputElement.prototype;
  Object.getOwnPropertyDescriptor(proto, "value")?.set?.call(element, next.value);
  try { element.setSelectionRange(next.caret, next.caret); } catch { /* number/email inputs have no selection */ }
  element.dispatchEvent(new (win(element.ownerDocument).InputEvent)("input", { bubbles: true, inputType, data }));
}

function insertText(text: string) {
  const { doc, element } = deepActive();
  if (isEditable(element)) {
    editValue(element, (v, s, e) => ({ value: v.slice(0, s) + text + v.slice(e), caret: s + text.length }), "insertText", text);
  } else if ((element as HTMLElement).isContentEditable) {
    if (!doc.execCommand("insertText", false, text)) throw new Error("The focused editable region rejected the text");
  } else throw new Error("No editable element is focused; pass id to focus one first");
}

function deleteText(forward: boolean) {
  const { element } = deepActive();
  if (!isEditable(element)) return;
  editValue(element, (v, s, e) => {
    const from = s !== e ? s : forward ? s : Math.max(0, s - 1);
    const to = s !== e ? e : forward ? Math.min(v.length, s + 1) : s;
    return { value: v.slice(0, from) + v.slice(to), caret: from };
  }, forward ? "deleteContentForward" : "deleteContentBackward", null);
}

export function type(params: Params) {
  allowKeys(params, "ui.type", ["text", "id"]);
  if (typeof params.text !== "string") throw new Error("text must be a string");
  if (Array.from(params.text).length > MAX_TYPED_CHARACTERS) throw new Error(`text is limited to ${MAX_TYPED_CHARACTERS} characters`);
  if (typeof params.id === "string") locate(params.id).element.closest<HTMLElement>(FOCUSABLE)?.focus();
  insertText(params.text);
  const { element } = deepActive();
  return { ok: true, id: element.id || undefined, tag: element.tagName.toLowerCase(), characters: Array.from(params.text).length };
}

interface KeySpec { key: string; code: string; mods: Mods }
function parseKey(spec: string, extra: unknown): KeySpec {
  const trailingPlus = spec.length > 1 && spec.endsWith("+");
  const tokens = (trailingPlus ? spec.slice(0, -1) : spec).split("+").filter(Boolean);
  if (trailingPlus) tokens.push("+");
  const name = tokens.pop();
  if (!name) throw new Error("key must name a key such as Enter, Tab or Ctrl+K");
  const mods = modifiersOf(extra, tokens);
  let key = KEY_ALIASES[name.toLowerCase()] ?? name;
  if (key.length > 1 && !/^(Enter|Tab|Escape|Backspace|Delete|Insert|Home|End|PageUp|PageDown|Arrow(Up|Down|Left|Right)|F([1-9]|1[0-2]))$/.test(key)) {
    const known = key[0].toUpperCase() + key.slice(1);
    throw new Error(`Unknown key ${JSON.stringify(name)}${known === name ? "" : `; did you mean ${known}?`}`);
  }
  if (key.length === 1 && mods.shift) key = key.toUpperCase();
  const code = /^[a-z]$/i.test(key) ? `Key${key.toUpperCase()}` : /^[0-9]$/.test(key) ? `Digit${key}` : key === " " ? "Space" : key;
  return { key, code, mods };
}

function moveFocus(backwards: boolean) {
  const { doc, element } = deepActive();
  const stops = Array.from(doc.querySelectorAll<HTMLElement>(FOCUSABLE)).filter(node => isVisible(node) && !node.matches(":disabled") && node.tabIndex >= 0);
  if (!stops.length) return;
  const index = stops.indexOf(element as HTMLElement);
  stops[(index + (backwards ? -1 : 1) + stops.length) % stops.length].focus();
}

function arrow(element: Element, key: string) {
  const forward = key === "ArrowRight" || key === "ArrowDown";
  const view = win(element.ownerDocument);
  if (element instanceof view.HTMLInputElement && element.type === "range") {
    if (key === "ArrowRight" || key === "ArrowUp") element.stepUp(); else element.stepDown();
  } else if (element instanceof view.HTMLSelectElement) {
    const options = Array.from(element.options);
    for (let i = element.selectedIndex + (forward ? 1 : -1); i >= 0 && i < options.length; i += forward ? 1 : -1) {
      if (!options[i].disabled) { element.selectedIndex = i; break; }
    }
  } else return;
  element.dispatchEvent(new view.Event("input", { bubbles: true }));
  element.dispatchEvent(new view.Event("change", { bubbles: true }));
}

/** What an untrusted key event cannot do on its own; limited to what a user expects from the key. */
function defaultAction(spec: KeySpec) {
  if (spec.mods.ctrl || spec.mods.alt || spec.mods.meta) return;
  const { doc, element } = deepActive();
  const press = (element as HTMLElement).click?.bind(element);
  if (spec.key.length === 1) {
    if (spec.key === " " && !isEditable(element) && !(element as HTMLElement).isContentEditable) press?.();
    else insertText(spec.key);
  } else if (spec.key === "Backspace") deleteText(false);
  else if (spec.key === "Delete") deleteText(true);
  else if (spec.key === "Tab") moveFocus(spec.mods.shift);
  else if (/^Arrow(Left|Right|Up|Down)$/.test(spec.key)) arrow(element, spec.key);
  else if (spec.key === "Enter") {
    if (element instanceof win(doc).HTMLTextAreaElement) insertText("\n");
    else if (element instanceof win(doc).HTMLInputElement && isEditable(element)) element.form?.requestSubmit();
    else if (element instanceof win(doc).HTMLButtonElement || element instanceof win(doc).HTMLAnchorElement) press?.();
    else if ((element as HTMLElement).isContentEditable) doc.execCommand("insertParagraph");
  }
}

export function key(params: Params) {
  allowKeys(params, "ui.key", ["key", "modifiers"]);
  if (typeof params.key !== "string") throw new Error("key must be a string such as Enter or Ctrl+K");
  const spec = parseKey(params.key, params.modifiers);
  const { doc, element } = deepActive();
  const init: KeyboardEventInit = {
    bubbles: true, cancelable: true, composed: true, view: win(doc), key: spec.key, code: spec.code,
    ctrlKey: spec.mods.ctrl, shiftKey: spec.mods.shift, altKey: spec.mods.alt, metaKey: spec.mods.meta,
  };
  const proceed = element.dispatchEvent(new (win(doc).KeyboardEvent)("keydown", init));
  if (proceed) defaultAction(spec);
  element.dispatchEvent(new (win(doc).KeyboardEvent)("keyup", init));
  return { ok: true, key: spec.key, target: element.id || element.tagName.toLowerCase(), defaultPrevented: !proceed };
}

/** `ui.set {id, checked}`: toggles a checkbox, or selects a radio, through a real click. */
export function setChecked(params: Params) {
  allowKeys(params, "ui.set", ["id", "checked"]);
  if (typeof params.id !== "string" || typeof params.checked !== "boolean") throw new Error("ui.set needs id and a boolean checked");
  const element = findElement(params.id);
  if (!element || !isVisible(element) || !(element instanceof win(element.ownerDocument).HTMLInputElement)
    || !["checkbox", "radio"].includes(element.type) || element.disabled) throw new Error("Control is missing, hidden, disabled, or not a checkbox or radio");
  if (element.type === "radio" && !params.checked) throw new Error("A radio button is cleared by choosing another in its group");
  if (element.checked !== params.checked) element.click();
  if (element.checked !== params.checked) throw new Error("The control did not accept the requested state");
  return { ok: true, id: params.id, checked: element.checked };
}
