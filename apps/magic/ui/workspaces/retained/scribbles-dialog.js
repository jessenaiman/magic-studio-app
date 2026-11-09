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
    this.templates = await motionCommand("scribbles.templates");
    const kinds = this.templates.kinds;
    const providers = this.templates.providers || [
      { id: "codex", label: "Codex (GPT-6 Luna · Local Templates)" },
      { id: "agy", label: "AGY (Gemini 3.8 Flash · Local Templates)" },
      { id: "flow", label: "Google Flow (Nano Banana · Browser-Agent)" }
    ];
    const scribRole = (this.templates.craft_ai?.roles || []).find((r) => r.role === "scribbles");
    const activeProvider = scribRole?.status?.Ready?.connection || "codex";
    const providerSelect = el(
      "select",
      { id: "scribbles-provider", "aria-label": "Drawing backend" },
      ...providers.map((p) => el("option", { value: p.id, selected: p.id === activeProvider ? "" : undefined }, p.label))
    );
    providerSelect.value = activeProvider;
    const kind = el("select", { id: "scribbles-kind", "aria-label": "Kind of drawing" }, ...Object.entries(kinds).map(([id, k]) => el("option", { value: id }, k.label)));
    const action = el("select", { id: "scribbles-action", "aria-label": "Action" }, ...Object.keys(kinds.actions.actions).map((id) => el("option", { value: id }, id)));
    const ref = el("input", { id: "scribbles-reference", class: "field", value: reference || "", "aria-label": "Reference image" });
    const prompt = el("textarea", { id: "scribbles-prompt", class: "field", rows: 4, "aria-label": "Character or drawing description" });
    const notes = el("textarea", { id: "scribbles-notes", class: "field", rows: 2, "aria-label": "Notes" });
    const link = el("a", { id: "scribbles-template-link", href: "#", class: "muted" });
    const layout = el("p", { id: "scribbles-layout", class: "muted" });
    const sent = el("textarea", { id: "scribbles-sent-prompt", class: "field", rows: 8, readonly: true, "aria-label": "Prompt sent to Scribbles" });
    const image = el("img", { id: "scribbles-image", alt: "Scribbles drawing", src: reference ? String(convertFileSrc(reference)) : "scribbles-cat-rest.png" });
    const status = el("p", { id: "scribbles-status", class: "muted", role: "status" }, "Choose a backend and kind, then Draw.");
    const actionField = new InspectorField("Action", action).node;
    providerSelect.onchange = async () => {
      try {
        await motionCommand("scribbles.connect", { provider: providerSelect.value });
        status.textContent = `Switched Scribbles backend to ${providerSelect.value}.`;
      } catch (err) {
        status.textContent = String(err);
      }
    };
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
      "data-commands": "scribbles.new scribbles.image scribbles.sheet scribbles.poses scribbles.poses_2x2 scribbles.parts_chroma scribbles.moppet scribbles.actions scribbles.layers scribbles.custom",
      onclick: async () => {
        if (this.busy)
          return;
        this.busy = true;
        draw.disabled = true;
        const prevSrc = image.src;
        image.src = "scribbles-work.gif";
        status.textContent = `Scribbles (${providerSelect.value}) is drawing… (a few minutes)`;
        if (typeof setScribblesLoadingWidget === "function")
          setScribblesLoadingWidget(true, "Scribbles is drawing…", "scribbles-work.gif");
        try {
          const params = {
            character,
            provider: providerSelect.value,
            reference: ref.value || undefined,
            prompt: prompt.value || undefined,
            notes: notes.value || undefined
          };
          if (kind.value === "actions")
            params.action = action.value;
          const result = await motionCommand(`scribbles.${kind.value}`, params);
          image.src = String(convertFileSrc(result.image));
          sent.value = result.scribbles?.prompt || prompt.value || "";
          const dims = result.width && result.height ? `${result.width} × ${result.height}` : `${result.scribbles?.canvas?.join(" × ") || "1024 × 1024"}`;
          status.textContent = `${dims}${result.scribbles?.canvas_matches === false ? ` · template asks ${result.scribbles.canvas.join(" × ")}` : ""} · ${result.image}`;
          status.append(fileButton({ path: result.image, name: String(result.image).split("/").pop() }));
          if (typeof animationLoadCharacter === "function" && character)
            await animationLoadCharacter(character);
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
    const openFlowBtn = el("button", {
      id: "scribbles-open-flow",
      class: "ghost",
      "data-commands": "scribbles.open",
      onclick: async () => {
        try {
          const res = await motionCommand("scribbles.open", { character, tool: "moppet", ingest: true });
          const count = res?.ingested?.registered_count ?? 0;
          status.textContent = count > 0 ? `Opened Flow Moppet Maker · ingested ${count} asset(s)` : "Opened Google Flow Moppet Maker";
          if (count > 0 && typeof animationLoadCharacter === "function" && character)
            await animationLoadCharacter(character);
        } catch (error) {
          status.textContent = String(error);
        }
      }
    }, "Open Flow Moppet Maker…");
    const ingestBtn = el("button", {
      id: "scribbles-ingest",
      class: "ghost",
      "data-commands": "scribbles.ingest",
      onclick: async () => {
        try {
          const res = await motionCommand("scribbles.ingest", { character });
          status.textContent = `Ingested ${res?.registered_count ?? 0} asset(s) for ${res?.character_id || character || "character"}.`;
          if (res?.character_image)
            image.src = String(convertFileSrc(res.character_image));
          if (typeof animationLoadCharacter === "function" && (res?.character_id || character))
            await animationLoadCharacter(res?.character_id || character);
        } catch (error) {
          status.textContent = String(error);
        }
      }
    }, "Ingest Downloads");
    const reassembleBtn = el("button", {
      id: "scribbles-reassemble",
      class: "ghost",
      "data-commands": "scribbles.reassemble",
      onclick: async () => {
        try {
          status.textContent = "Reassembling #00FF00 parts sheet and rigging…";
          const res = await motionCommand("scribbles.reassemble", { character, rig: true });
          status.textContent = `Reassembled ${res?.parts_count ?? 6} layers and rigged ${character || "character"}.`;
          if (typeof animationLoadCharacter === "function" && character)
            await animationLoadCharacter(character);
        } catch (error) {
          status.textContent = String(error);
        }
      }
    }, "Reassemble & Rig Parts");
    const close = el("button", { id: "scribbles-close", class: "ghost", onclick: () => {
      if (animationReview.canLeave())
        this.root.close();
    } }, "Close");
    const actionsRow = el("div", { class: "motion-path-actions" }, draw, openFlowBtn, ingestBtn, reassembleBtn);
    const fields = el("div", { class: "panel-fields" }, new InspectorField("Backend", providerSelect).node, new InspectorField("Kind", kind).node, actionField, new InspectorField("Reference image", ref).node, new InspectorField("Character / drawing", prompt).node, new InspectorField("Notes", notes).node, layout, link, actionsRow, el("details", {}, el("summary", {}, "Prompt sent to Scribbles"), sent));
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
