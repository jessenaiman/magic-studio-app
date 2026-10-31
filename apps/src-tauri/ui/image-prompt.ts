// A separate image composer. It never reads or changes the chat/provider configuration.
class ImagePromptController {
  drafts = new Map();

  selectedPartTarget() {
    if (!motion?.character || !motion?.selected || !motion?.meta?.layers) return null;
    const layer = motion.meta.layers.find((part) => part.id === motion.selected);
    if (!layer?.file || !state.project) return null;
    const root = state.project.slice(0, state.project.lastIndexOf('/'));
    const prefix = `${root}/`;
    const original = motion.assets?.find((asset) => asset.id === motion.character)?.path;
    const sourceFile = layer.source_file || layer.file;
    const localPng = path => typeof path === 'string' && path.startsWith(prefix) && /\.png$/i.test(path);
    // Rig asset paths are JSON. Use its declared full-scene source PNG and the
    // selected part PNG; never treat the derived assembled preview as authority.
    const fullSource = localPng(original) ? original : motion.meta.original_reference;
    if (!localPng(fullSource) || !localPng(sourceFile)) return null;
    const references = [...new Set([fullSource, sourceFile])].map(path => path.slice(prefix.length));
    return {
      project: state.project, assetId: motion.character, partId: layer.id, name: layer.label || layer.id,
      references, file: sourceFile, original: fullSource,
      rigSha256: motion.meta.rig_sha256, partSha256: layer.image_sha256, revision: motion.meta.image_revision,
      sourceSize: layer.source_size,
    };
  }

  // The parts panel uses this to display only the candidate generated for the captured part.
  candidateForPart(assetId, partId) {
    const draft = this.drafts.get(state.project);
    const target = draft?.candidateTarget;
    if (!draft?.result?.image || target?.assetId !== assetId || target?.partId !== partId) return null;
    return { ...target, image: draft.result.image, result: draft.result };
  }

  // A created asset stays successful even when refreshing its current view fails.
  async promptCompleted(project, draft, result, captured) {
    draft.result = result;
    draft.candidateTarget = captured;
    draft.refreshError = '';
    if (state.project !== project) return;
    try {
      const data = await invoke('project_state', { path: project });
      if (state.project !== project) return;
      state.data = data;
      if (state.view === 'animation') await animationRefresh();
      if (state.project !== project) return;
      renderBlocks();
      await acFeedback.report(null, null, { stage: 'image_block_rendered', command: 'image.generate', asset_id: result.asset_id, receipt: result.receipt, captured_part: captured?.partId || null });
    } catch (error) { draft.refreshError = `Artwork created; view refresh failed: ${String(error)}`; }
  }

  draftFor(project) {
    let draft = this.drafts.get(project);
    if (!draft) {
      draft = { prompt: '', model: 'gpt-6-luna', busy: false, result: null, error: '', startedAt: null, target: null, candidateTarget: null, timer: null };
      this.drafts.set(project, draft);
    }
    return draft;
  }

  targetLabel(draft) {
    return draft.target ? `${draft.target.name} · ${draft.target.partId}` : '';
  }

  paneState(draft) {
    return draft.error ? 'failed' : draft.busy ? 'working' : draft.result ? 'ready' : 'idle';
  }

  setStatus(status, draft, targetLabel, elapsed) {
    status.dataset.signal = draft.error ? '3' : draft.result ? '2' : '0';
    if (draft.busy) status.textContent = `Creating artwork · ${elapsed()}s${targetLabel ? ` · ${targetLabel}` : ''}`;
    else if (draft.error) status.textContent = draft.error;
    else if (draft.result) status.textContent = `Ready · ${draft.result.width} × ${draft.result.height} · Review candidate${draft.candidateTarget ? ` · ${draft.candidateTarget.name} (${draft.candidateTarget.partId})` : ''}${draft.refreshError ? ` · ${draft.refreshError}` : ''}`;
    else status.textContent = 'Describe the artwork you want to create.';
    status.title = draft.result ? `Completed in ${draft.completedElapsed ?? 0}s · ${draft.result.native_tool_receipt_verified ? 'native receipt verified' : 'receipt unavailable'}` : status.textContent;
  }

  resultElements(draft) {
    if (!draft.result?.image) return [];
    return [
      el('img', { class: 'image-result', src: convertFileSrc(draft.result.image), alt: 'Generated candidate artwork · needs visual review' }),
      el('button', { class: 'ghost', id: 'image-open-result', onclick: () => openPath(draft.result.image) }, 'Open generated image'),
    ];
  }

  async generate(draft, project, prompt, generate, updateStatus, elapsed) {
    if (!draft.prompt.trim()) { prompt.focus(); return; }
    if (draft.busy) return;
    // Freeze the target and reference at invocation time; later selection changes cannot retarget this job.
    if (motion?.selected && !this.selectedPartTarget()) {
      draft.error = 'This part has no project-local full source reference. Generation was not started.';
      updateStatus();
      return;
    }
    const captured = this.selectedPartTarget();
    draft.target = captured;
    draft.busy = true; draft.error = ''; draft.refreshError = ''; draft.result = null; draft.startedAt = performance.now();
    generate.disabled = true;
    updateStatus();
    draft.timer = setInterval(updateStatus, 250);
    const submitted: {prompt: any, model: any, quality: string, references?: any} = { prompt: draft.prompt, model: draft.model, quality: 'auto' };
    if (captured) submitted.references = captured.references;
    try {
      const result = await invoke('engine_command', { id: 'image.generate', params: submitted });
      draft.completedElapsed = elapsed();
      await this.promptCompleted(project, draft, result, captured);
    } catch (error) { draft.completedElapsed = elapsed(); draft.error = String(error); }
    finally {
      clearInterval(draft.timer); draft.timer = null; draft.busy = false;
      if (state.project === project) renderBlocks();
    }
  }

  createModelInput(draft, updateCommand) {
    const model = el('input', { id: 'image-model', value: draft.model, list: 'image-model-options', 'aria-label': 'Doodle artwork model', title: 'Selected Codex model; pixel dimensions are determined by its native image tool' });
    model.addEventListener('input', () => {
      draft.model = model.value;
      updateCommand();
      if (!draft.busy && !draft.result && !draft.error) {
        const hdr = document.getElementById?.('magic-image-toggle');
        if (hdr) hdr.title = `Doodle · Artwork · ${draft.model}`;
      }
    });
    return model;
  }

  renderBlock() {
    const project = state.project;
    const draft = this.draftFor(project);
    const prompt = el('textarea', { id: 'image-prompt', rows: '3', placeholder: 'Describe the artwork or part repair…', 'aria-label': 'Doodle artwork prompt' });
    prompt.value = prompt.defaultValue = draft.prompt;
    const command = el('code', { id: 'image-command' });
    const updateCommand = () => {
      command.textContent = magicCommandText('image.generate', {
        prompt: draft.prompt, model: draft.model, quality: 'auto',
        ...(draft.target?.references ? { references: draft.target.references } : {}),
      }, project);
    };
    prompt.addEventListener('input', () => { draft.prompt = prompt.value; updateCommand(); });
    const model = this.createModelInput(draft, updateCommand);
    const models = el('datalist', { id: 'image-model-options' }, el('option', { value: 'gpt-6-luna' }), el('option', { value: 'gpt-6-sol' }));
    updateCommand();
    const elapsed = () => Math.max(0, Math.floor((performance.now() - draft.startedAt) / 1000));
    const targetLabel = this.targetLabel(draft);
    const status = el('p', { id: 'image-status', role: 'status', 'aria-live': 'polite', class: 'why' });
    const updateStatus = () => this.setStatus(status, draft, targetLabel, elapsed);
    updateStatus();
    const generate = el('button', { id: 'image-generate', 'data-commands': 'image.generate', 'aria-label': 'Doodle: generate artwork candidate', onclick: async () => { await this.generate(draft, project, prompt, generate, updateStatus, elapsed); } }, draft.busy ? 'Generating…' : 'Generate candidate');
    generate.disabled = draft.busy;
    const copy = el('button', { class: 'ghost', id: 'image-copy-command', title: 'Copy the exact image command without executing it', onclick: async () => {
      try { await navigator.clipboard.writeText(command.textContent); toast('Image command copied · nothing executed'); }
      catch { toast('Clipboard unavailable · select the command text'); }
    } }, 'Copy command');
    const settings = el('div', { id: 'image-settings' }, el('label', { for: 'image-model' }, 'Model'), model, models,
      el('details', {}, el('summary', {}, 'CLI command'), command, copy));
    settings.hidden = $('gaffer-config-toggle')?.getAttribute('aria-expanded') !== 'true';
    const body = [el('label', { for: 'image-prompt' }, 'Prompt'), prompt, status, generate, settings, ...this.resultElements(draft)];
    window.uiRegistry.fact({ id: 'magic-image-pane', project: state.project, state: this.paneState(draft), reason: draft.error || null, source: 'artwork result' });
    return el('div', { id: 'image-tools-body' }, ...body);
  }
}

const imagePromptController = new ImagePromptController();
const imageDrafts = imagePromptController.drafts;

function selectedPartTarget() {
  return imagePromptController.selectedPartTarget();
}
function imageCandidateForPart(assetId, partId) {
  return imagePromptController.candidateForPart(assetId, partId);
}
function imagePromptBlock() {
  return imagePromptController.renderBlock();
}
