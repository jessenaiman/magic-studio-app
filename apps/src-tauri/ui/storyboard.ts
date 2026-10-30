// ---- Storyboard: title cards ------------------------------------------------------------

class StoryboardViewController {
  savePromise: Promise<any> | null = null;
  selectedPlacement: string | null = null;

  pageSize(): [number, number] {
    return state.data?.storyboard?.page_size || state.data?.storyboard?.layout?.page_size || [960, 540];
  }

  renderEmpty(cards: any) {
    cards.replaceChildren();
    delete cards.dataset.document;
    $("sb-page").textContent = "Storyboard";
    cards.append(projectSetupCard());
  }

  renderChips(sb: any, pages: string[], chips: any) {
    const first = (sb.first_scene_page || 2) - 1;
    const nav = pages.map((_: string, i: number) => [sb.page_titles?.[i] || sb.scene_titles?.[i - first] || `Page ${i + 1}`, i]);
    state.pageTitles = Object.fromEntries(nav.map(([label, page]: any) => [page, label]));
    for (const [i, [label, page]] of nav.entries()) {
      const active = page === state.page;
      const isChapter = i < first;
      chips.append(el("button", {
        class: `chip${active ? " active" : ""}${isChapter ? " chapter" : " scene"}`,
        role: "tab", "aria-selected": String(active), title: label,
        onclick: () => this.showPage(page),
      }, label));
    }
    chips.querySelector(".active")?.scrollIntoView({ block: "nearest", inline: "center", behavior: "smooth" });
    $("sb-page").textContent = `${state.page + 1} / ${pages.length}`;
  }

  appendPageLinks(page: any, sb: any, pageIndex: number, size: [number, number]) {
    for (const [j, link] of (sb.layout?.links || []).entries()) {
      if (link.page !== pageIndex + 1) continue;
      const [x, y, r, b] = link.rect;
      const linkButton = el("button", {
        id: `sb-link-${pageIndex}-${j}`, class: "sb-link",
        title: `Go to page ${link.to_page}`, "aria-label": `Go to page ${link.to_page}`,
        onclick: () => this.showPage(link.to_page - 1),
      });
      linkButton.style.setProperty("--x", `${x / size[0] * 100}%`);
      linkButton.style.setProperty("--y", `${y / size[1] * 100}%`);
      linkButton.style.setProperty("--w", `${(r - x) / size[0] * 100}%`);
      linkButton.style.setProperty("--h", `${(b - y) / size[1] * 100}%`);
      page.append(linkButton);
    }
  }

  bindPageDropTarget(page: any, pageIndex: number, size: [number, number]) {
    page.addEventListener("dragover", (e: DragEvent) => {
      e.preventDefault();
      page.classList.add("drop-target");
      if (e.dataTransfer) e.dataTransfer.dropEffect = "copy";
    });
    page.addEventListener("dragleave", () => page.classList.remove("drop-target"));
    page.addEventListener("drop", async (e: DragEvent) => {
      e.preventDefault();
      page.classList.remove("drop-target");
      const id = e.dataTransfer?.getData("application/x-magicstudio-asset");
      if (!id) return;
      const r = page.getBoundingClientRect();
      try {
        await designCommand("storyboard.place_asset", {
          asset: id, page: pageIndex + 1,
          x: (e.clientX - r.left) / r.width * size[0] - 80,
          y: (e.clientY - r.top) / r.height * size[1] - 80,
          width: 160,
        });
      } catch (error) { toast(String(error)); }
    });
  }

  bindWrapScroll(wrap: any, chips: any, totalPages: number) {
    wrap.onscroll = () => {
      let current = 0;
      const top = wrap.getBoundingClientRect().top;
      [...wrap.children].forEach((page: any, i: number) => {
        if (page.getBoundingClientRect().top <= top + wrap.clientHeight * 0.35) current = i;
      });
      if (state.page === current) return;
      state.page = current;
      state.storyboardScene = (state.data?.storyboard_contents?.chapters || []).find((scene: any) => scene.page === current + 1)?.target || null;
      chips.querySelectorAll("[role=tab]").forEach((tab: any, i: number) => {
        tab.classList.toggle("active", i === current);
        tab.setAttribute("aria-selected", String(i === current));
      });
      $("sb-page").textContent = `${current + 1} / ${totalPages}`;
      renderBlocks();
      updateAppFooter();
    };
  }

  renderDocumentSheets(wrap: any, sb: any, pages: string[], chips: any) {
    const key = JSON.stringify([state.project, pages, sb.layout, sb.page_size]);
    if (wrap.dataset.document === key) return;
    wrap.dataset.document = key;
    wrap.replaceChildren();
    wrap.classList.add("sb-page-wrap", "document-pages");
    const size = this.pageSize();
    pages.forEach((path: string, i: number) => {
      const page = el("div", { id: `sb-sheet-${i}`, class: "sb-sheet" });
      page.style.setProperty("--ar", `${size[0]}/${size[1]}`);
      page.append(el("img", { src: convertFileSrc(path), alt: `Storyboard page ${i + 1}`, loading: "lazy" }));
      this.appendPageLinks(page, sb, i, size);
      this.bindPageDropTarget(page, i, size);
      wrap.append(page);
    });
    this.bindWrapScroll(wrap, chips, pages.length);
    $(`sb-sheet-${state.page}`)?.scrollIntoView({ block: "start", behavior: "instant" });
  }

  render() {
    updateAppFooter();
    const sb = state.data?.storyboard;
    document.body.classList.toggle("movie-document", Boolean(sb?.deck));
    const pages = sb?.previews || [];
    const chips = $("sb-chips");
    chips.replaceChildren();
    const cards = $("sb-cards");
    $("sb-empty")?.remove();
    for (const id of ["sb-prev", "sb-next", "sb-open-deck", "sb-save-version"]) $(id).disabled = !pages.length;
    if (!pages.length) {
      this.renderEmpty(cards);
      return;
    }
    state.page = Math.max(0, Math.min(state.page, pages.length - 1));
    this.renderChips(sb, pages, chips);
    this.renderDocumentSheets(cards, sb, pages, chips);
    this.renderText();
    this.renderPlacements();
  }

  showPage(n: number) {
    const total = state.data?.storyboard?.previews?.length || 0;
    state.page = Math.max(0, Math.min(total - 1, n));
    state.storyboardScene = (state.data?.storyboard_contents?.chapters || []).find((scene: any) => scene.page === state.page + 1)?.target || null;
    this.render();
    renderBlocks();
    $(`sb-sheet-${state.page}`)?.scrollIntoView({ block: "start", behavior: "instant" });
  }

  ensureTextFieldNode(id: string, field: any, page: any) {
    let node = $(id);
    if (node) return node;
    const label = field.field.replaceAll("_", " ");
    node = el("textarea", {
      id, class: "sb-text", title: `Edit ${label} · saved when you finish typing`,
      "aria-label": `Scene ${field.scene || "page " + field.page} ${label}`,
      "data-commands": "storyboard.edit", spellcheck: "true",
    });
    node.addEventListener("input", () => {
      if (node.value === node.defaultValue) projectView.textDrafts.delete(id);
      else projectView.textDrafts.set(id, { page: field.page, field: field.field, value: node.value });
      updateAppFooter();
    });
    node.addEventListener("change", () => this.save().catch((error) => toast(String(error))));
    page.append(node);
    return node;
  }

  styleTextFieldNode(node: any, field: any, size: [number, number]) {
    const [x, y, w, h] = field.rect;
    const put = (k: string, v: string) => node.style.setProperty(k, v);
    const height = field.name ? h : Math.max(h, 50);
    const fontSize = field.name ? (field.font_size || 12) : (field.field === "lyrics" ? 27 : 21);
    put("--x", `${x / size[0] * 100}%`);
    put("--y", `${y / size[1] * 100}%`);
    put("--w", `${w / size[0] * 100}%`);
    put("--h", `${height / size[1] * 100}%`);
    put("--font", `${fontSize / size[0] * 100}cqw`);
    if (field.name) {
      node.dataset.named = "";
      put("--pad", `${2 / size[0] * 100}cqw`);
      put("--field-fill", field.fill);
      put("--field-ink", field.text_color);
    }
  }

  renderText() {
    const size = this.pageSize();
    const kept = new Set<string>();
    for (const field of state.data?.design?.text_fields || []) {
      const page = $(`sb-sheet-${field.page - 1}`);
      if (!page) continue;
      const id = `sb-text-${field.page}-${field.id}`;
      kept.add(id);
      const node = this.ensureTextFieldNode(id, field, page);
      const draft = projectView.textDrafts.get(id);
      if (!draft) node.value = node.defaultValue = field.value || "";
      else if (node.value !== draft.value) node.value = draft.value;
      this.styleTextFieldNode(node, field, size);
    }
    document.querySelectorAll(".sb-text").forEach((node: any) => { if (!kept.has(node.id)) node.remove(); });
  }

  updateSaveControls() {
    const disabled = projectView.pending > 0 || Boolean(state.data?.design?.error) || (!state.data?.design?.dirty && !projectView.textDrafts.size);
    for (const id of ["sb-save-version", "storyboard-save"]) if ($(id)) $(id).disabled = disabled;
  }

  save() {
    if (this.savePromise) return this.savePromise;
    const project = state.project;
    this.savePromise = (async () => {
      let result;
      do { result = await this.persist(); }
      while (state.project === project && projectView.textDrafts.size);
      return result;
    })().finally(() => {
      this.savePromise = null;
      this.updateSaveControls();
    });
    return this.savePromise;
  }

  async persist() {
    const project = state.project;
    if (state.data?.design?.error) {
      throw new Error(`Cannot save until the editable storyboard is readable: ${state.data.design.error}`);
    }
    const edits = [...projectView.textDrafts.entries()];
    for (const [, edit] of edits) await designCommand("storyboard.edit", { ...edit, save: false });
    const params = state.data?.storyboard?.draft ? { draft: state.data.storyboard.manifest } : {};
    const result = await designCommand("storyboard.save_version", params);
    if (state.project !== project) return result;
    for (const [id, edit] of edits) {
      if (projectView.textDrafts.get(id)?.value === edit.value) projectView.textDrafts.delete(id);
    }
    this.renderText();
    renderBlocks();
    updateAppFooter();
    toast("Storyboard saved");
    return result;
  }

  scaleFactor(hx: number, hy: number, dx: number, dy: number, w: number, h: number) {
    if (hx === 0) return 1 + hy * dy / h;
    if (hy === 0) return 1 + hx * dx / w;
    return 1 + (hx * dx / w + hy * dy / h) / 2;
  }

  scaledRect(hx: number, hy: number, x: number, y: number, w: number, h: number, nw: number, nh: number) {
    const rx = hx < 0 ? x + w - nw : hx === 0 ? x + (w - nw) / 2 : x;
    const ry = hy < 0 ? y + h - nh : hy === 0 ? y + (h - nh) / 2 : y;
    return [rx, ry, nw, nh];
  }

  bindScaleHandle(handle: any, item: any, page: any, size: [number, number], hx: number, hy: number, place: any) {
    const [x, y, w, h] = item.rect;
    handle.addEventListener("pointerdown", (e: PointerEvent) => {
      e.preventDefault();
      e.stopPropagation();
      this.selectedPlacement = item.id;
      handle.setPointerCapture(e.pointerId);
      const r = page.getBoundingClientRect(), start = [e.clientX, e.clientY];
      let rect = [x, y, w, h];
      handle.onpointermove = (event: PointerEvent) => {
        const dx = (event.clientX - start[0]) * size[0] / r.width;
        const dy = (event.clientY - start[1]) * size[1] / r.height;
        const factor = this.scaleFactor(hx, hy, dx, dy, w, h);
        const nw = Math.max(20, Math.min(size[0], w * factor)), nh = nw * h / w;
        rect = this.scaledRect(hx, hy, x, y, w, h, nw, nh);
        place(rect[0], rect[1], nw, nh);
      };
      handle.onpointerup = async () => {
        handle.onpointermove = null;
        handle.onpointerup = null;
        try {
          await designCommand("storyboard.place_asset", {
            asset: item.asset, page: item.page, placement: item.id,
            x: rect[0], y: rect[1], width: rect[2],
          });
        } catch (error) { toast(String(error)); }
      };
    });
  }

  bindMovePlacement(node: any, item: any, page: any, size: [number, number], place: any) {
    const [x, y, w] = item.rect;
    node.addEventListener("pointerdown", (e: PointerEvent) => {
      this.selectedPlacement = item.id;
      e.preventDefault();
      node.setPointerCapture(e.pointerId);
      const r = page.getBoundingClientRect(), start = [e.clientX, e.clientY];
      let nx = x, ny = y;
      node.onpointermove = (event: PointerEvent) => {
        nx = x + (event.clientX - start[0]) * size[0] / r.width;
        ny = y + (event.clientY - start[1]) * size[1] / r.height;
        place(nx, ny);
      };
      node.onpointerup = async () => {
        node.onpointermove = null;
        node.onpointerup = null;
        try {
          await designCommand("storyboard.place_asset", {
            asset: item.asset, page: item.page, x: nx, y: ny, width: w, placement: item.id,
          });
        } catch (error) { toast(String(error)); }
      };
    });
  }

  renderPlacementItem(item: any, size: [number, number]) {
    const page = $(`sb-sheet-${item.page - 1}`);
    if (!page) return;
    const [x, y, w, h] = item.rect;
    const node = el("div", { id: item.id, class: "design-placement", title: "Drag to move · grab an edge to scale" });
    const place = (px: number, py: number, pw?: number, ph?: number) => {
      node.style.setProperty("--x", `${100 * px / size[0]}%`);
      node.style.setProperty("--y", `${100 * py / size[1]}%`);
      if (pw != null && ph != null) {
        node.style.setProperty("--w", `${100 * pw / size[0]}%`);
        node.style.setProperty("--h", `${100 * ph / size[1]}%`);
      }
    };
    place(x, y, w, h);
    node.append(el("img", { src: convertFileSrc(item.path), alt: "Placed design asset", draggable: "false" }));
    for (const [hx, hy] of [[-1, -1], [0, -1], [1, -1], [-1, 0], [1, 0], [-1, 1], [0, 1], [1, 1]]) {
      const handle = el("button", { class: "design-handle", title: "Drag to scale", "aria-label": "Scale placed image", "data-hx": hx, "data-hy": hy });
      this.bindScaleHandle(handle, item, page, size, hx, hy, place);
      node.append(handle);
    }
    this.bindMovePlacement(node, item, page, size, place);
    const remove = el("button", {
      class: "design-remove", title: "Remove this placement", "aria-label": "Remove this placement",
      onclick: () => designCommand("storyboard.remove_placement", { placement: item.id }).catch((e) => toast(String(e))),
    }, "×");
    remove.style.setProperty("--x", `${100 * (x + w) / size[0]}%`);
    remove.style.setProperty("--y", `${100 * y / size[1]}%`);
    page.append(node, remove);
  }

  renderPlacements() {
    const entries = state.data?.design?.placements || [];
    const size = this.pageSize();
    document.querySelectorAll(".design-placement,.design-remove").forEach((n) => n.remove());
    this.updateSaveControls();
    $("sb-save-version").hidden = !(state.data?.storyboard?.pdf_fields || state.data?.storyboard?.deck);
    if (!entries.some((p: any) => p.id === this.selectedPlacement)) {
      this.selectedPlacement = entries.at(-1)?.id || null;
    }
    for (const item of entries) this.renderPlacementItem(item, size);
  }
}

const storyboardController = new StoryboardViewController();

function renderStoryboard() { storyboardController.render(); }
function showPage(n: number) { storyboardController.showPage(n); }
function storyboardPageSize() { return storyboardController.pageSize(); }
function renderStoryboardText() { storyboardController.renderText(); }
function updateStoryboardSaveControls() { storyboardController.updateSaveControls(); }
function saveStoryboard() { return storyboardController.save(); }
async function persistStoryboard() { return storyboardController.persist(); }
function renderDesignPlacements() { storyboardController.renderPlacements(); }

$("sb-save-version").addEventListener("click", () => saveStoryboard().catch((error) => toast(String(error))));
$("sb-prev").addEventListener("click", () => showPage(state.page - 1));
$("sb-next").addEventListener("click", () => showPage(state.page + 1));
$("sb-open-deck").addEventListener("click", () => state.data?.storyboard?.deck && openPath(state.data.storyboard.deck));
window.addEventListener("keydown", (e: KeyboardEvent) => {
  if (state.view !== "project" || (e.target as HTMLElement)?.closest("input, textarea")) return;
  if (e.key === "ArrowRight") showPage(state.page + 1);
  if (e.key === "ArrowLeft") showPage(state.page - 1);
});
