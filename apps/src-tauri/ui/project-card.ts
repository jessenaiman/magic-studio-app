// A project card presents catalog data. Navigation and history belong to its host.
class ProjectCardView {
  callbacks: {open: (project: any, action?: any) => any, locate: (project: any) => any, forget: (project: any) => any};
  project: any;
  status: any;

  constructor(project: any, callbacks: {open: (project: any, action?: any) => any, locate: (project: any) => any, forget: (project: any) => any}) {
    this.project = project;
    this.callbacks = callbacks;
    this.status = el('p', { class: 'project-card-status', role: 'status' }, project.readiness?.next_step || 'Open project');
  }

  run(action: () => any) {
    return async event => {
      const button = event.currentTarget;
      button.disabled = true;
      try { await action(); }
      catch (error) { this.status.textContent = String(error.message || error); this.status.dataset.error = 'true'; }
      finally { button.disabled = false; }
    };
  }

  renderThumbnail() {
    const thumbnail = el('div', { class: 'project-card-thumbnail', 'aria-hidden': 'true' });
    if (this.project.thumb) {
      thumbnail.append(el('img', {
        src: convertFileSrc(this.project.thumb),
        alt: '',
        onerror: () => thumbnail.replaceChildren(el('span', { html: FILM })),
      }));
    } else {
      thumbnail.append(el('span', { html: FILM }));
    }
    return thumbnail;
  }

  renderActions(canOpen: boolean) {
    const actions = el('div', { class: 'project-card-actions' });
    if (canOpen) {
      for (const action of this.project.readiness?.quick_actions || []) {
        actions.append(el('button', {
          class: 'ghost',
          disabled: !action.enabled,
          'aria-label': `${action.label} · ${this.project.name}`,
          'data-commands': 'project.open',
          title: action.enabled ? `${action.label} · ${this.project.name}` : this.project.readiness.next_step,
          onclick: this.run(() => this.callbacks.open(this.project, action)),
        }, action.label));
      }
    } else {
      actions.append(el('button', {
        class: 'ghost',
        'aria-label': `Locate ${this.project.name}`,
        'data-commands': 'project.open project.catalog',
        onclick: this.run(() => this.callbacks.locate(this.project)),
      }, 'Locate…'));
    }
    return actions;
  }

  render() {
    const availability = this.project.availability || (this.project.exists ? 'available' : 'missing');
    const canOpen = availability === 'available';
    const labels = { available: 'Available', missing: 'Location unavailable', unreadable: 'Cannot read project' };
    const heading = el('button', {
      class: 'project-card-heading',
      title: this.project.path,
      'aria-label': `Open ${this.project.name}`,
      'data-commands': 'project.open',
      disabled: !canOpen,
      onclick: this.run(() => this.callbacks.open(this.project)),
    },
      el('strong', {}, this.project.name),
      el('span', { class: 'project-card-path', title: this.project.folder }, this.project.folder),
      el('span', { class: 'project-card-availability' }, labels[availability] || 'Check project'));
    return el('article', { box: `recent:${this.project.path}`, class: 'project-card', 'data-availability': availability },
      el('div', { class: 'project-card-header' }, this.renderThumbnail(), heading,
        el('button', {
          class: 'icon project-card-forget',
          'aria-label': `Remove ${this.project.name} from Recent`,
          'data-commands': 'project.catalog',
          title: 'Remove from Recent (files stay)',
          html: CROSS,
          onclick: this.run(() => this.callbacks.forget(this.project)),
        })),
      this.status, this.renderActions(canOpen));
  }
}

function projectCard(project: any, callbacks: {open: (project: any, action?: any) => any, locate: (project: any) => any, forget: (project: any) => any}) {
  return new ProjectCardView(project, callbacks).render();
}
