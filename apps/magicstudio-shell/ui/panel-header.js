// Shared panel chrome. Creative lettering belongs to each view's content.
class PanelHeader {
  constructor({id, title, description, titleId, icon = null, collapsible = false, actions = null}) {
    const entry = uiConfig.elements[id] || {};
    this.label = el('span', {id:titleId, class:'panel-title'});
    this.actions = actions ? el('div', {class:'panel-actions'}, ...actions) : null;
    this.element = el(collapsible ? 'summary' : 'div', {id},
      icon ? el('span', {class:'ico', html:icon, 'aria-hidden':'true'}) : null,
      this.label,
      this.actions,
      collapsible ? el('span', {class:'chev', html:ICONS.chev, 'aria-hidden':'true'}) : null);
    this.rename(entry.label || title, description || entry.description);
  }
  rename(title, description = title) {
    this.label.textContent = title;
    this.element.title = description;
    this.element.setAttribute('aria-label', title);
  }
}
