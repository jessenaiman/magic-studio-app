class InspectorNumberField extends InspectorField {
  range;
  readout;
  dial;
  options;
  marker;
  known;
  constructor(options) {
    const value = options.value ?? Math.max(options.min, Math.min(options.max, 0));
    const input = el("input", {
      id: options.id,
      class: "control-range",
      type: "range",
      step: "any",
      min: Math.min(options.min, value),
      max: Math.max(options.max, value),
      value,
      "aria-label": options.label,
      "aria-orientation": options.control === "vertical" ? "vertical" : "horizontal"
    });
    super(options.label, input, "div");
    this.options = options;
    this.known = options.value !== null;
    this.range = input;
    this.readout = el("output", { id: `${options.id}-value`, for: options.id, class: "craft-number-readout" });
    this.dial = options.control === "dial" ? this.createDial() : null;
    this.marker = this.dial?.querySelector(".craft-dial-marker") ?? null;
    this.node.className = "inspector-field craft-number";
    this.node.dataset.control = options.control || "horizontal";
    this.node.replaceChildren(el("label", { class: "craft-number-title", for: options.id }, el("span", { class: "craft-control-label" }, options.label), this.readout), el("span", { class: "craft-number-body" }, this.dial, input));
    input.addEventListener("input", () => {
      this.known = true;
      this.sync();
      options.change?.(Number(input.value));
    });
    input.addEventListener("keydown", (event) => this.key(event));
    new MutationObserver(() => this.sync()).observe(input, { attributes: true, attributeFilter: ["disabled"] });
    this.sync();
  }
  setValue(value, notify = false) {
    if (!Number.isFinite(value) || notify && this.input.disabled)
      return;
    this.input.min = String(Math.min(this.options.min, value));
    this.input.max = String(Math.max(this.options.max, value));
    this.input.value = String(value);
    this.known = true;
    this.sync();
    if (notify)
      this.input.dispatchEvent(new Event("input", { bubbles: true }));
  }
  sync() {
    const value = Number(this.input.value), unit = this.options.unit || "";
    this.readout.textContent = this.known ? `${Number(value.toFixed(2))}${unit}` : "—";
    this.readout.title = this.known ? `${value}${unit}` : "Animated channel";
    this.node.dataset.disabled = String(this.input.disabled);
    if (!this.dial)
      return;
    this.dial.setAttribute("aria-valuemin", this.input.min);
    this.dial.setAttribute("aria-valuemax", this.input.max);
    this.dial.setAttribute("aria-valuenow", String(value));
    this.dial.setAttribute("aria-valuetext", this.readout.textContent);
    this.dial.setAttribute("aria-disabled", String(this.input.disabled));
    this.dial.tabIndex = this.input.disabled ? -1 : 0;
    const fraction = (value - Number(this.input.min)) / (Number(this.input.max) - Number(this.input.min) || 1);
    this.marker?.setAttribute("transform", `rotate(${-135 + 270 * fraction} 24 24)`);
  }
  key(event) {
    const delta = { ArrowUp: 1, ArrowRight: 1, ArrowDown: -1, ArrowLeft: -1, PageUp: 10, PageDown: -10 }[event.key];
    if (!delta || this.input.disabled)
      return;
    event.preventDefault();
    const next = Number(this.input.value) + delta * this.options.step * (event.shiftKey ? 10 : 1);
    this.setValue(Math.max(this.options.min, Math.min(this.options.max, Number(next.toFixed(6)))), true);
    this.input.dispatchEvent(new Event("change", { bubbles: true }));
  }
  createDial() {
    const dial = el("span", {
      id: `${this.options.id}-dial`,
      class: "craft-dial",
      role: "slider",
      tabindex: 0,
      "aria-label": `${this.options.label} dial`,
      title: "Drag up or down · Shift for fine adjustment · arrow keys to step",
      html: '<svg viewBox="0 0 48 48" aria-hidden="true"><circle class="craft-dial-track" cx="24" cy="24" r="19"/><circle class="craft-dial-face" cx="24" cy="24" r="14"/><path class="craft-dial-marker" d="M24 13v7"/></svg>'
    });
    dial.addEventListener("keydown", (event) => this.key(event));
    let start = null;
    dial.addEventListener("pointerdown", (event) => {
      if (this.input.disabled)
        return;
      event.preventDefault();
      dial.focus();
      start = { y: event.clientY, value: Number(this.input.value) };
      dial.setPointerCapture(event.pointerId);
    });
    dial.addEventListener("pointermove", (event) => {
      if (!start)
        return;
      const value = start.value + (start.y - event.clientY) * this.options.step * (event.shiftKey ? 0.1 : 1);
      this.setValue(Number(Math.min(this.options.max, Math.max(this.options.min, value)).toFixed(6)), true);
    });
    dial.addEventListener("lostpointercapture", () => {
      if (start)
        this.input.dispatchEvent(new Event("change", { bubbles: true }));
      start = null;
    });
    return dial;
  }
}

class InspectorToggle extends InspectorField {
  constructor(id, label, checked, change) {
    const input = el("input", { id, type: "checkbox", role: "switch", class: "craft-switch", "aria-label": label });
    super(label, input);
    input.checked = checked;
    input.defaultChecked = checked;
    this.node.className = "inspector-field field-row craft-toggle";
    if (change)
      input.addEventListener("change", () => change(input.checked));
  }
}
