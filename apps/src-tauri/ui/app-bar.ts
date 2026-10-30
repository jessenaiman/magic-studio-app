// Stable view IDs are routing contracts. Rename a workspace here, independently of its ID.
interface WorkspaceView { id: string; label: string; description: string; health?: boolean }

const WORKSPACE_VIEWS: WorkspaceView[] = [
  {id:'project', label:'Storyboard', description:'Plan your song and storyboard'},
  {id:'animation', label:'Animation', description:'Animate objects and polish their motion'},
  {id:'design', label:'Lyrics', description:'Design lettering and motion timed to the lyrics'},
  {id:'studio', label:'Studio', description:'Assemble and preview your film'},
  {id:'skills', label:'Resources', description:'Visual references and reusable examples for your videos'},
  {id:'analytics', label:'Mayhem', description:'App observations and Backstage', health:true}
];

class WorkspaceNavigation {
  readonly root: HTMLElement;
  readonly views: WorkspaceView[];
  readonly buttons: HTMLElement[];
  readonly picker: HTMLSelectElement;

  constructor(root: HTMLElement, views: WorkspaceView[], navigate: (id: string, action?: unknown) => void) {
    this.root = root;
    this.views = views;
    this.buttons = views.map(view => {
      const button = el('button', {id:`tab-${view.id}`, class:'tab', 'data-view':view.id,
        role:'tab', 'aria-selected':'false', 'aria-controls':`view-${view.id}`,
        tabindex:-1, title:view.description, onclick:()=>navigate(view.id, acFeedback.actions.get(button))},
        el('span', {class:'tab-label'}, view.label));
      if (view.health) {
        button.dataset.health = 'unknown';
        button.prepend(el('span', {class:'navigation-health', 'aria-hidden':'true'}));
        button.append(el('small', {id:'mayhem-health', class:'navigation-health-label'}, 'Unknown'));
      }
      return button;
    });
    root.replaceChildren(...this.buttons);
    this.picker = el('select', {id:'workspace-select', 'aria-label':'Workspace', onchange:event=>navigate(event.target.value)},
      ...views.map(view=>el('option', {value:view.id}, view.label)));
    root.after(this.picker);
    root.addEventListener('keydown', event => this.onKey(event));
    this.select(state.view);
  }
  select(id) {
    this.picker.value = id;
    for (const button of this.buttons) {
      const selected = button.dataset.view === id;
      button.classList.toggle('active', selected);
      button.setAttribute('aria-selected', String(selected));
      button.tabIndex = selected ? 0 : -1;
    }
    const active = this.buttons.find(button => button.dataset.view === id);
    if (active && !document.body.classList.contains('at-home')) active.scrollIntoView({block:'nearest', inline:'nearest'});
  }
  readiness(allowed, reason) {
    for (const view of this.views) {
      const button = this.buttons.find(item => item.dataset.view === view.id);
      // Navigation stays available; the engine still owns operation prerequisites.
      button.disabled = false;
      button.title = ['animation','design'].includes(view.id) && !allowed && reason
        ? `${view.description} · Storyboard: ${reason}` : view.description;
    }
  }
  onKey(event) {
    const index = this.buttons.indexOf(event.target.closest('[role=tab]'));
    if (index < 0 || !['ArrowLeft','ArrowRight','Home','End'].includes(event.key)) return;
    event.preventDefault();
    event.stopPropagation();
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? this.buttons.length - 1
      : (index + (event.key === 'ArrowRight' ? 1 : -1) + this.buttons.length) % this.buttons.length;
    this.buttons[next].focus();
  }
}
window.workspaceNavigation = new WorkspaceNavigation($('workspace-tabs'), WORKSPACE_VIEWS, setView);
