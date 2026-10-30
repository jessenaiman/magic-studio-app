// A project card presents catalog data. Navigation and history belong to its host.
function projectCard(project, { open, locate, forget }) {
  const availability = project.availability || (project.exists ? 'available' : 'missing');
  const canOpen = availability === 'available';
  const labels = { available: 'Available', missing: 'Location unavailable', unreadable: 'Cannot read project' };
  const status = el('p', { class: 'project-card-status', role: 'status' }, project.readiness?.next_step || 'Open project');
  const run = action => async event => {
    const button = event.currentTarget;
    button.disabled = true;
    try { await action(); }
    catch (error) { status.textContent = String(error.message || error); status.dataset.error = 'true'; }
    finally { button.disabled = false; }
  };
  const thumbnail = el('div', { class: 'project-card-thumbnail', 'aria-hidden': 'true' });
  if (project.thumb) thumbnail.append(el('img', { src: convertFileSrc(project.thumb), alt: '',
    onerror: () => thumbnail.replaceChildren(el('span', { html: FILM })) }));
  else thumbnail.append(el('span', { html: FILM }));
  const heading = el('button', { class: 'project-card-heading', title: project.path,
    'aria-label': `Open ${project.name}`, 'data-commands': 'project.open', disabled: !canOpen, onclick: run(() => open(project)) },
    el('strong', {}, project.name),
    el('span', { class: 'project-card-path', title: project.folder }, project.folder),
    el('span', { class: 'project-card-availability' }, labels[availability] || 'Check project'));
  const actions = el('div', { class: 'project-card-actions' });
  if (canOpen) {
    for (const action of project.readiness?.quick_actions || []) {
      actions.append(el('button', { class: 'ghost', disabled: !action.enabled,
        'aria-label': `${action.label} · ${project.name}`, 'data-commands': 'project.open',
        title: action.enabled ? `${action.label} · ${project.name}` : project.readiness.next_step,
        onclick: run(() => open(project, action)) }, action.label));
    }
  } else {
    actions.append(el('button', { class: 'ghost', 'aria-label': `Locate ${project.name}`,
      'data-commands': 'project.open project.catalog',
      onclick: run(() => locate(project)) }, 'Locate…'));
  }
  return el('article', { box: `recent:${project.path}`, class: 'project-card', 'data-availability': availability },
    el('div', { class: 'project-card-header' }, thumbnail, heading,
      el('button', { class: 'icon project-card-forget', 'aria-label': `Remove ${project.name} from Recent`,
        'data-commands': 'project.catalog',
        title: 'Remove from Recent (files stay)', html: CROSS, onclick: run(() => forget(project)) })),
    status, actions);
}
