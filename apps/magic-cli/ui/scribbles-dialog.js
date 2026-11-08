class ScribblesDialog {
  root = null;
  templates = null;
  busy = false;
  drawingLayout = null;
  rigHost = null;
  rigMode = false;
  character = null;
  async open(character, reference, review = false) {
    this.character = character;
    if (this.root?.open) {
      if (!animationReview.canLeave())
        return;
      this.root.close();
    }
    this.templates = this.templates || await motionCommand("scribbles.templates");
    const kinds = this.templates.kinds;
    const kind = el("select", { id: "scribbles-kind", "aria-label": "Kind of drawing" }, ...Object.entries(kinds).map(([id, k]) => el("option", { value: id }, k.label)));
    const action = el("select", { id: "scribbles-action", "aria-label": "Action" }, ...Object.keys(kinds.actions.actions).map((id) => el("option", { value: id }, id)));
    const ref = el("input", { id: "scribbles-reference", class: "field", value: reference || "", "aria-label": "Reference image" });
    const prompt = el("textarea", { id: "scribbles-prompt", class: "field", rows: 4, "aria-label": "Character or drawing description" });
    const notes = el("textarea", { id: "scribbles-notes", class: "field", rows: 2, "aria-label": "Notes" });
    const link = el("a", { id: "scribbles-template-link", href: "#", class: "muted" });
    const layout = el("p", { id: "scribbles-layout", class: "muted" });
    const sent = el("textarea", { id: "scribbles-sent-prompt", class: "field", rows: 8, readonly: true, "aria-label": "Prompt sent to Scribbles" });
    const image = el("img", { id: "scribbles-image", alt: "Scribbles drawing", src: reference ? String(convertFileSrc(reference)) : "scribbles-cat-rest.png" });
    const status = el("p", { id: "scribbles-status", class: "muted", role: "status" }, "Choose a kind, then Draw.");
    const actionField = new InspectorField("Action", action).node;
    const describe = () => {
      const k = kinds[kind.value];
      actionField.hidden = kind.value !== "actions";
      link.textContent = `How this is done: ${k.reference}`;
      link.onclick = (e) => {
        e.preventDefault();
        window.__TAURI__.opener?.openUrl?.(k.reference);
      };
      layout.textContent = `${k.canvas[0]} × ${k.canvas[1]} px · ${k.layout}`;
    };
    kind.onchange = describe;
    describe();
    const draw = el("button", {
      id: "scribbles-draw",
      class: "start",
      "data-commands": "scribbles.new scribbles.image scribbles.sheet scribbles.poses scribbles.actions scribbles.custom",
      onclick: async () => {
        if (this.busy)
          return;
        this.busy = true;
        draw.disabled = true;
        const prevSrc = image.src;
        image.src = "scribbles-work.gif";
        status.textContent = "Scribbles is drawing… (a few minutes)";
        if (typeof setScribblesLoadingWidget === "function")
          setScribblesLoadingWidget(true, "Scribbles is drawing…", "scribbles-work.gif");
        try {
          const params = { character, reference: ref.value || undefined, prompt: prompt.value || undefined, notes: notes.value || undefined };
          if (kind.value === "actions")
            params.action = action.value;
          const result = await motionCommand(`scribbles.${kind.value}`, params);
          image.src = String(convertFileSrc(result.image));
          sent.value = result.scribbles.prompt;
          status.textContent = `${result.width} × ${result.height}${result.scribbles.canvas_matches ? "" : ` · template asks ${result.scribbles.canvas.join(" × ")}`} · ${result.image}`;
          status.append(fileButton({ path: result.image, name: String(result.image).split("/").pop() }));
        } catch (error) {
          image.src = prevSrc || "scribbles-cat-alert.png";
          status.textContent = String(error);
        } finally {
          this.busy = false;
          draw.disabled = false;
          if (typeof setScribblesLoadingWidget === "function")
            setScribblesLoadingWidget(false);
        }
      }
    }, "Draw");
    const close = el("button", { id: "scribbles-close", class: "ghost", onclick: () => {
      if (animationReview.canLeave())
        this.root.close();
    } }, "Close");
    const fields = el("div", { class: "panel-fields" }, new InspectorField("Kind", kind).node, actionField, new InspectorField("Reference image", ref).node, new InspectorField("Character / drawing", prompt).node, new InspectorField("Notes", notes).node, layout, link, draw, el("details", {}, el("summary", {}, "Prompt sent to Scribbles"), sent));
    this.drawingLayout = el("div", { class: "artwork-review-layout" }, el("div", { class: "artwork-review-canvas" }, image), fields);
    this.rigHost = el("div", { id: "scribbles-rig-review", hidden: true });
    const mode = el("button", { id: "scribbles-review-3d", class: "ghost", onclick: () => this.setMode(!this.rigMode) }, "3D animation review");
    this.root = el("dialog", { id: "scribbles-dialog", class: "editor-dialog artwork-review", "aria-label": "Scribbles" }, new PanelHeader({ id: "scribbles-head", title: "Scribbles", actions: [mode, close] }).element, this.drawingLayout, this.rigHost, status);
    this.root.addEventListener("cancel", (event) => {
      if (this.busy || !animationReview.canLeave())
        event.preventDefault();
    });
    this.root.addEventListener("close", () => {
      animationReview.dispose();
      this.rigMode = false;
      this.root.remove();
    });
    document.body.append(this.root);
    this.root.showModal();
    if (review)
      await this.setMode(true);
  }
  async setMode(review) {
    if (!animationReview.canLeave())
      return;
    this.rigMode = review;
    this.drawingLayout.hidden = review;
    this.rigHost.hidden = !review;
    $("scribbles-status").hidden = review;
    $("scribbles-review-3d").textContent = review ? "Drawing / sheets" : "3D animation review";
    if (review) {
      animationReview.character = this.character || animationReview.character;
      await animationReview.open(this.rigHost);
    } else
      animationReview.dispose();
  }
}
const scribblesDialog = new ScribblesDialog;
function openScribbles(character = motion.character, reference = motion.meta?.view?.drawing || null, review = false) {
  return scribblesDialog.open(character, reference, review).catch((error) => toast(String(error)));
}
