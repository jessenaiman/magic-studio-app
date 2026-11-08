class PanelHeader {
  label;
  actions;
  element;
  constructor({ id, title, description, titleId, icon = null, collapsible = false, actions = null, layout = null }) {
    const entry = uiConfig.elements[id] || {};
    this.label = el("span", { id: titleId, class: "panel-title" });
    this.actions = actions ? el("div", { class: "panel-actions" }, ...actions) : null;
    this.element = el(collapsible ? "summary" : "div", { id }, icon ? el("span", { class: "ico", html: icon, "aria-hidden": "true" }) : null, this.label, this.actions, collapsible ? el("span", { class: "chev", html: ICONS.chev, "aria-hidden": "true" }) : null);
    if (layout) this.element.dataset.layout = layout;
    this.rename(entry.label || title, description || entry.description);
  }
  rename(title, description = title) {
    this.label.textContent = title;
    this.element.title = description;
    this.element.setAttribute("aria-label", title);
  }
}

class InspectorField {
  input;
  node;
  constructor(label, input, container = "label") {
    this.input = input;
    this.node = el(container, { class: "inspector-field field-row" }, el("span", { class: "craft-control-label" }, label), input);
  }
}
const TABLIST_KEYS = ["ArrowLeft", "ArrowRight", "Home", "End"];
function tablistTabs(list) {
  return [...list.querySelectorAll("[role=tab]")].filter((tab) => !tab.disabled);
}
function tablistStep(key, index, count) {
  if (key === "Home")
    return 0;
  if (key === "End")
    return count - 1;
  return (index + (key === "ArrowRight" ? 1 : -1) + count) % count;
}
function tablistSelect(list, next) {
  const label = list.getAttribute("aria-label");
  tablistTabs(list)[next].click();
  const live = list.isConnected || !label ? list : document.querySelector(`[role=tablist][aria-label="${CSS.escape(label)}"]`);
  if (live)
    tablistTabs(live)[next]?.focus();
}
document.addEventListener("keydown", (event) => {
  if (!TABLIST_KEYS.includes(event.key))
    return;
  const tab = event.target.closest?.("[role=tab]");
  const list = tab?.closest("[role=tablist]");
  const tabs = list ? tablistTabs(list) : [];
  const index = tabs.indexOf(tab);
  if (index < 0)
    return;
  event.preventDefault();
  event.stopPropagation();
  tablistSelect(list, tablistStep(event.key, index, tabs.length));
});
