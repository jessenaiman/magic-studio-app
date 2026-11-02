// Scribbles: ask for a drawing from one of the templates (`magic-cli scribbles <kind>`) and preview
// the result full size before anything uses it. The dialog only calls commands and shows answers.

class ScribblesDialog {
  root: any = null;
  templates: any = null;
  busy = false;

  async open(character: string | null, reference: string | null) {
    this.templates = this.templates || await motionCommand('scribbles.templates');
    const kinds = this.templates.kinds;
    const kind = el('select', { id: 'scribbles-kind', 'aria-label': 'Kind of drawing' },
      ...Object.entries(kinds).map(([id, k]: [string, any]) => el('option', { value: id }, k.label)));
    const action = el('select', { id: 'scribbles-action', 'aria-label': 'Action' },
      ...Object.keys(kinds.actions.actions).map((id) => el('option', { value: id }, id)));
    const ref = el('input', { id: 'scribbles-reference', class: 'field', value: reference || '', 'aria-label': 'Reference image' });
    const prompt = el('textarea', { id: 'scribbles-prompt', class: 'field', rows: 4, 'aria-label': 'Character or drawing description' });
    const notes = el('textarea', { id: 'scribbles-notes', class: 'field', rows: 2, 'aria-label': 'Notes' });
    const link = el('a', { id: 'scribbles-template-link', href: '#', class: 'muted' });
    const layout = el('p', { id: 'scribbles-layout', class: 'muted' });
    const sent = el('textarea', { id: 'scribbles-sent-prompt', class: 'field', rows: 8, readonly: true, 'aria-label': 'Prompt sent to Scribbles' });
    const image = el('img', { id: 'scribbles-image', alt: 'Scribbles drawing' });
    const status = el('p', { id: 'scribbles-status', class: 'muted', role: 'status' }, 'Choose a kind, then Draw.');
    const actionField = new InspectorField('Action', action).node;
    const describe = () => {
      const k = kinds[kind.value];
      actionField.hidden = kind.value !== 'actions';
      link.textContent = `How this is done: ${k.reference}`;
      link.onclick = (e: Event) => { e.preventDefault(); window.__TAURI__.opener?.openUrl?.(k.reference); };
      layout.textContent = `${k.canvas[0]} × ${k.canvas[1]} px · ${k.layout}`;
    };
    kind.onchange = describe;
    describe();
    const draw = el('button', { id: 'scribbles-draw', class: 'start', 'data-commands': 'scribbles.new scribbles.image scribbles.sheet scribbles.poses scribbles.actions scribbles.custom',
      onclick: async () => {
        if (this.busy) return;
        this.busy = true;
        draw.disabled = true;
        status.textContent = 'Scribbles is drawing… (a few minutes)';
        try {
          const params: any = { character, reference: ref.value || undefined, prompt: prompt.value || undefined, notes: notes.value || undefined };
          if (kind.value === 'actions') params.action = action.value;
          const result = await motionCommand(`scribbles.${kind.value}`, params);
          image.src = String(convertFileSrc(result.image));
          sent.value = result.scribbles.prompt;
          status.textContent = `${result.width} × ${result.height}${result.scribbles.canvas_matches ? '' : ` · template asks ${result.scribbles.canvas.join(' × ')}`} · ${result.image}`;
          status.append(fileButton({ path: result.image, name: String(result.image).split('/').pop() }));
        } catch (error) { status.textContent = String(error); }
        finally { this.busy = false; draw.disabled = false; }
      } }, 'Draw');
    const close = el('button', { id: 'scribbles-close', class: 'ghost', onclick: () => this.root.close() }, 'Close');
    const fields = el('div', { class: 'panel-fields' },
      new InspectorField('Kind', kind).node, actionField, new InspectorField('Reference image', ref).node,
      new InspectorField('Character / drawing', prompt).node, new InspectorField('Notes', notes).node,
      layout, link, draw, el('details', {}, el('summary', {}, 'Prompt sent to Scribbles'), sent));
    this.root = el('dialog', { id: 'scribbles-dialog', class: 'editor-dialog artwork-review', 'aria-label': 'Scribbles' },
      new PanelHeader({ id: 'scribbles-head', title: 'Scribbles', actions: [close] }).element,
      el('div', { class: 'artwork-review-layout' }, el('div', { class: 'artwork-review-canvas' }, image), fields), status);
    this.root.addEventListener('cancel', (event: Event) => { if (this.busy) event.preventDefault(); });
    this.root.addEventListener('close', () => this.root.remove());
    document.body.append(this.root);
    this.root.showModal();
  }
}

const scribblesDialog = new ScribblesDialog();
function openScribbles(character: string | null = motion.character, reference: string | null = motion.meta?.view?.drawing || null) {
  return scribblesDialog.open(character, reference).catch((error: any) => toast(String(error)));
}
