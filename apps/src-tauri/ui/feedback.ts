// Bounded rendered feedback for this application only. Never read input values or agent chat text.
class FeedbackController {
  ack = 0;
  action: any = null;
  actions = new WeakMap();
  errors: any[] = [];
  feedbackTimer: any = null;
  last: any = null;
  pending = 0;
  phase = 'booting';

  constructor() {
    window.acFeedback = this as any;
    this.bindEvents();
    this.report('booting');
    // Refresh observation health without creating activity or acknowledging new engine work.
    setInterval(() => this.report(), 1000);
  }

  editedFields(): string[] {
    const changed = [...document.querySelectorAll('input:not([type=password]),textarea,[contenteditable=true]')]
      .filter((n: any) => n.getBoundingClientRect().width && n.getBoundingClientRect().height && !n.closest('#agent-log') && n.id !== 'motion-scrub')
      .filter((n: any) => n.isContentEditable ? !!n.textContent.trim() : n.type === 'checkbox' || n.type === 'radio' ? n.checked !== n.defaultChecked : n.value !== n.defaultValue)
      .map((n: any) => n.id || n.tagName.toLowerCase());
    if (typeof projectView !== 'undefined') {
      changed.push(...projectView.textDrafts.keys());
      if (state.data?.design?.dirty === true) changed.push('storyboard-design');
    }
    if (typeof theatreFeedback === 'function' && theatreFeedback().live_edits === 'needs_save') changed.push('theatre-motion');
    return changed;
  }

  paneRendered(): boolean {
    if (typeof state === 'undefined') return false;
    const view = state.view === 'studio' && typeof serverUrl === 'function' && serverUrl() ? 'project' : state.view;
    const pane = document.querySelector(state.project || state.creating ? '#view-' + view : '#view-home');
    return !!pane && pane.getBoundingClientRect().width > 0 && pane.getBoundingClientRect().height > 0 && getComputedStyle(pane).display !== 'none' && pane.textContent.trim().length > 0;
  }

  chatSelector() {
    if (typeof agent === "undefined" || !agent.loaded) return null;
    const value = (id: string) => (document.getElementById(id) as HTMLInputElement | null)?.value || null;
    return {agent: value('agent-command'), provider: value('agent-provider'), model: value('agent-model'), source: 'rendered selector; may include unsaved edits'};
  }

  snapshot() {
    const panels = window.companionBoxes ? window.companionBoxes.panels() : null;
    return {
      companions: window.magicCompanions?.snapshot() || null,
      panels,
      theatre: typeof theatreFeedback === 'function' ? theatreFeedback() : null,
      chat_selector: this.chatSelector(),
      rendered: this.paneRendered(),
      phase: this.phase,
      errors: this.errors.slice(-8),
      pending_invokes: this.pending,
      ack_engine_revision: this.ack,
      project: typeof state === 'undefined' ? null : state.project,
      view: typeof state === 'undefined' ? null : document.body.classList.contains('at-home') ? 'home' : state.view,
      draft_fields: this.editedFields(),
      last_frontend_command: this.last,
    };
  }

  async report(phase?: string | null, ack?: number | null, event?: any) {
    if (phase) this.phase = phase;
    if (ack) this.ack = Math.max(this.ack, ack);
    try {
      await window.__TAURI__.core.invoke('ui_report', {report: {...this.snapshot(), ...(event ? {event} : {})}});
    } catch (e) {
      console.error('UI feedback failed', String(e));
    }
  }

  async acknowledge(revision: number | null, project: string | null) {
    const frameObserved = await Promise.race([
      new Promise(resolve => requestAnimationFrame(() => resolve(true))),
      new Promise(resolve => setTimeout(() => resolve(false), 800)),
    ]);
    if (state.project !== project) throw new Error('Render project mismatch; acknowledgement withheld');
    const current = this.snapshot();
    if (!current.rendered || current.errors.length) throw new Error('Frontend render incomplete; acknowledgement withheld');
    if (!frameObserved) {
      await this.report('ready', null, {stage: 'render_deferred', project, reason: 'No animation frame observed; engine revision acknowledgement withheld'});
      return;
    }
    await this.report('ready', revision);
  }

  handleClick(e: any) {
    const n = e.target.closest('button,[role=button],[role=tab]');
    if (!n) return;
    const control = n.id || n.getAttribute('aria-label') || n.getAttribute('title') || n.textContent.trim().slice(0, 80) || 'unknown-control';
    const request_id = `ui-${Date.now()}-${Math.random().toString(16).slice(2, 8)}`;
    const navigation = n.matches('#project-details,#sb-prev,#sb-next,.sb-link,#sb-chips [role=tab],.tab[data-view]');
    const previous = this.action;
    const errors = this.errors.length;
    e.acAction = {
      control,
      request_id,
      box: n.closest("[data-companion-rail]")?.getAttribute("data-companion-rail") || n.closest("[data-box-id]")?.getAttribute("data-box-id") || null,
    };
    this.actions.set(n, e.acAction);
    this.action = e.acAction;
    // Static handlers run during this dispatch too; invoke captures this context before awaiting.
    queueMicrotask(() => { if (this.action === e.acAction) this.action = previous; });
    this.report(null, null, {
      stage: 'ui_click',
      control,
      request_id,
      interaction: navigation ? 'navigation' : 'control',
      verified_commands: (n.dataset.commands || '').split(' ').filter(Boolean),
      mapping: navigation ? 'existing navigation handler' : n.dataset.commands ? 'verified handler annotation' : 'unknown',
    });
    if (navigation && !n.matches('.tab[data-view]')) {
      requestAnimationFrame(() => {
        const current = this.snapshot();
        if (current.rendered && this.errors.length === errors) {
          this.report(null, null, {stage: 'ui_navigation_applied', control, request_id, view: current.view, page: state.page + 1});
        }
      });
    }
  }

  bindEvents() {
    window.addEventListener('error', e => {
      this.errors.push({message: String(e.message).slice(0, 300), source: String(e.filename).split('/').pop(), line: e.lineno});
      this.report('error');
    });
    window.addEventListener('unhandledrejection', e => {
      this.errors.push({message: String(e.reason).slice(0, 300)});
      this.report('error');
    });
    document.addEventListener('input', () => {
      clearTimeout(this.feedbackTimer);
      this.feedbackTimer = setTimeout(() => this.report(), 150);
    });
    document.addEventListener('click', e => this.handleClick(e), true);
  }
}

const feedbackController = new FeedbackController();
function editedFields(): string[] { return feedbackController.editedFields(); }
function paneRendered(): boolean { return feedbackController.paneRendered(); }
function chatSelector() { return feedbackController.chatSelector(); }
function feedbackSnapshot() { return feedbackController.snapshot(); }
