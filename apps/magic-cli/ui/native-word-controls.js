// Discover the supported native property names/types; keep this editor deliberately small.
let nativeWordProperties;
function nativeWordControls(anim) {
  const lineId = wordsPanel.line, project = state.project;
  const design = wordsDesign(anim, lineId);
  const index = wordsStageController.resolveActiveItemIndex(design, wordsPanel.picked);
  const item = design.items[index];
  const panel = el("section", { id: "native-word-controls", class: "panel-body native-word-controls", "aria-label": "Native word motion controls" });
  if (!item) return panel;
  const words = wordsLineWords(anim, lineId), label = item.word_ids.map(id => words.find(word => word.word_id === id)?.text).filter(Boolean).join(" ");
  panel.append(el("strong", {}, `Word motion · ${label}`));
  const status = el("p", { id: "native-word-motion-status", class: "muted", role: "status" }, "Reading native controls…");
  panel.append(status);
  nativeWordProperties ||= wordsCommand("words.motion.properties");
  void nativeWordProperties.then(result => {
    if (!panel.isConnected || state.project !== project) return;
    const properties = result.properties || [];
    const selected = item.native_motion || { property: "transform/rotation", from: 0, speed: 0, duration: 2 };
    const property = el("select", { id: "word-motion-property", "aria-label": "Native transform property" }, ...properties.map(prop => el("option", { value: prop.path }, prop.name)));
    property.value = selected.property;
    const fields = el("div", { class: "native-motion-fields" });
    let inputs;
    const input = (id, label, value, min, max, step) => {
      const field = el("input", { id, class: "field", type: "number", min, max, step, "aria-label": label });
      field.value = field.defaultValue = String(value);
      fields.append(el("label", {}, label, field));
      return field;
    };
    const renderFields = () => {
      fields.replaceChildren(); inputs = {};
      const config = property.value === selected.property ? selected : {};
      inputs.delay = input("word-motion-delay", "Delay after word · seconds", config.delay ?? 0, 0, 7.95, .05);
      inputs.duration = input("word-motion-duration", "Duration · seconds", config.duration ?? 2, .05, 8, .05);
      if (property.value === "transform/rotation") {
        inputs.from = input("word-motion-from", "Start angle offset · degrees", config.from ?? 0, -14400, 14400, 1);
        inputs.speed = input("word-motion-speed", "Rotation speed · degrees / second", config.speed ?? 0, -1800, 1800, 5);
      } else if (property.value === "transform/opacity") {
        inputs.from = input("word-motion-from", "From · %", config.from ?? 100, 0, 100, 1);
        inputs.to = input("word-motion-to", "To · %", config.to ?? 100, 0, 100, 1);
      } else {
        const scale = property.value === "transform/scale", initial = scale ? 100 : 0;
        inputs.from = [0,1].map(axis => input(`word-motion-from-${axis}`, `From ${axis ? "Y" : "X"} · ${scale ? "%" : "px offset"}`, config.from?.[axis] ?? initial, scale ? 1 : -1080, scale ? 400 : 1080, 1));
        inputs.to = [0,1].map(axis => input(`word-motion-to-${axis}`, `To ${axis ? "Y" : "X"} · ${scale ? "%" : "px offset"}`, config.to?.[axis] ?? initial, scale ? 1 : -1080, scale ? 400 : 1080, 1));
      }
    };
    property.onchange = renderFields; renderFields();
    const apply = el("button", { id: "word-motion-apply", class: "ghost", onclick: async () => {
      if (![...fields.querySelectorAll("input")].every(field => field.checkValidity() && field.value.trim())) { status.textContent = "Enter valid values for every field."; return; }
      apply.disabled = true;
      try {
        const read = field => Array.isArray(field) ? field.map(input => Number(input.value)) : Number(field.value);
        const spec = { property: property.value, delay: read(inputs.delay), duration: read(inputs.duration), from: read(inputs.from), ...(inputs.speed ? { speed: read(inputs.speed) } : { to: read(inputs.to) }) };
        const curve = await wordsCommand("words.motion.curve", spec);
        if (state.project !== project || wordsPanel.selected !== anim.animation_id || wordsPanel.line !== lineId) throw new Error("Selection changed; no motion was applied.");
        wordsDesignEdit(anim, lineId, next => {
          const target = next.items.find(candidate => JSON.stringify(candidate.word_ids) === JSON.stringify(item.word_ids));
          if (!target) throw new Error("The selected word group changed");
          target.native_motion = curve.config; target.motion_keys = curve.motion_keys; target.accent = target.word_ids[0];
        });
      } catch (error) { status.textContent = String(error); } finally { apply.disabled = false; }
    } }, "Apply motion to selection");
    status.textContent = item.native_motion ? "Native transform curve · Save to persist" : "Choose a property. Rotation speed may be negative to reverse direction.";
    panel.insertBefore(el("label", {}, "Property", property), status);
    panel.insertBefore(fields, status); panel.append(apply);
  }).catch(error => { status.textContent = `Native controls unavailable: ${error}`; nativeWordProperties = null; });
  return panel;
}
