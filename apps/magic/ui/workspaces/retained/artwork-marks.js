function artworkSvg(name, attributes) {
  const node = document.createElementNS("http://www.w3.org/2000/svg", name);
  for (const [key, value] of Object.entries(attributes)) node.setAttribute(key, value);
  return node;
}

function artworkMarkEditor(view, draft, onChange) {
  const [width, height] = view.size;
  const svg = artworkSvg("svg", { viewBox: `0 0 ${width} ${height}`, role: "group", "aria-label": "Part outlines and joints" });
  svg.append(artworkSvg("image", { href: animationFileUrl(view.drawing), width, height }));
  const radius = Math.max(width, height) * 0.014;
  const pointString = outline => outline.map(point => point.join(",")).join(" ");
  for (const part of draft.parts) {
    const outline = artworkSvg("polygon", { points: pointString(part.outline), class: "artwork-outline" });
    svg.append(outline);
    const handles = [...part.outline.map((point, index) => ({ point, name: `Outline point ${index + 1}` })),
      { point: part.joint, name: "Joint" }];
    for (const { point, name } of handles) {
      const handle = artworkSvg("circle", { cx: point[0], cy: point[1], r: radius, tabindex: 0,
        class: name === "Joint" ? "artwork-joint" : "artwork-handle", "aria-label": `${part.id} ${name}` });
      const title = artworkSvg("title", {});
      title.textContent = `${part.id} · ${name}`;
      handle.append(title);
      function move(x, y) {
        point[0] = Math.round(Math.max(0, Math.min(width, x)));
        point[1] = Math.round(Math.max(0, Math.min(height, y)));
        handle.setAttribute("cx", point[0]);
        handle.setAttribute("cy", point[1]);
        outline.setAttribute("points", pointString(part.outline));
        onChange();
      }
      handle.addEventListener("pointerdown", event => handle.setPointerCapture(event.pointerId));
      handle.addEventListener("pointermove", event => {
        if (!handle.hasPointerCapture(event.pointerId)) return;
        const matrix = svg.getScreenCTM();
        if (!matrix) return;
        const target = new DOMPoint(event.clientX, event.clientY).matrixTransform(matrix.inverse());
        move(target.x, target.y);
      });
      handle.addEventListener("keydown", event => {
        const delta = { ArrowLeft: [-1, 0], ArrowRight: [1, 0], ArrowUp: [0, -1], ArrowDown: [0, 1] }[event.key];
        if (!delta) return;
        event.preventDefault();
        const step = event.shiftKey ? 10 : 1;
        move(point[0] + delta[0] * step, point[1] + delta[1] * step);
      });
      svg.append(handle);
    }
  }
  return el("div", { class: "artwork-mark-editor" }, svg);
}
