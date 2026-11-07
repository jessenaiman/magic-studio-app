renderWorkspaceNav("settings");
const root = document.getElementById("settings-root");
async function load() {
  const { invoke } = window.__TAURI__.core;
  const schema = await invoke("execute_command", { id: "prefs.schema", params: {} });
  root.replaceChildren(...schema.categories.map(cat => el("section", { class: "panel-body" },
    el("h2", {}, cat.title),
    ...cat.fields.map(f => fieldControl(f, invoke)))));
}
function fieldControl(f, invoke) {
  const id = `settings.${f.key}`;
  const kind = f.kind.type;
  let control;
  if (kind === "bool") control = el("input", { id, type: "checkbox", class: "craft-switch", checked: f.value === true });
  else if (kind === "int" || kind === "float") control = el("input", { id, type: "range", class: "control-range", min: f.kind.min, max: f.kind.max, step: kind === "int" ? 1 : 0.1, value: f.value ?? f.kind.min });
  else if (kind === "choice") control = el("select", { id }, ...f.kind.choices.map(c => el("option", { value: c.value, selected: f.value === c.value ? true : null }, c.label)));
  else control = el("input", { id, type: "text", value: f.value ?? "" });
  const node = el("div", { class: "inspector-field craft-number" },
    el("label", { for: id, class: "craft-number-title" }, el("span", { class: "craft-control-label" }, f.label), el("output", { class: "craft-number-readout" }, String(f.value ?? ""))),
    el("span", { class: "craft-number-body" }, control));
  control.addEventListener("change", async () => {
    const value = kind === "bool" ? control.checked : kind === "int" || kind === "float" ? Number(control.value) : control.value;
    await invoke("execute_command", { id: "prefs.set", params: { key: f.key, value } });
  });
  return node;
}
load().catch(e => root.textContent = String(e));
