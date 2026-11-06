function applyAttribute(node, key, value) {
  if (key === "class")
    node.className = value;
  else if (key === "html")
    node.innerHTML = value;
  else if (key.startsWith("on"))
    node.addEventListener(key.slice(2), (event) =>
      typeof runListener === "function" ? runListener(value, event) : value(event));
  else if (typeof value === "boolean" && key in node)
    node[key] = value;
  else
    node.setAttribute(key, value);
}

function registerElement(node, boxId) {
  const entry = boxId && typeof uiConfig !== "undefined" ? uiConfig.elements[boxId] : null;
  if (entry)
    applyEntry(node, entry, boxId);
  if (boxId) {
    node.setAttribute("data-box-id", boxId);
    const project = typeof state === "undefined" ? null : state.project;
    window.companionBoxes?.register(boxId, { project, label: entry?.label || null, look: entry?.look || null, density: entry?.density || null });
  }
  window.uiRegistry?.registerElement(node);
}

function el(tag, attrs = {}, ...children) {
  const node = document.createElement(tag);
  let box = null;
  for (const [key, value] of Object.entries(attrs)) {
    if (value === null || value === undefined)
      continue;
    if (key === "box")
      box = value;
    else
      applyAttribute(node, key, value);
  }
  for (const child of children.flat())
    if (child != null)
      node.append(child);
  registerElement(node, box || attrs.id || null);
  return node;
}
