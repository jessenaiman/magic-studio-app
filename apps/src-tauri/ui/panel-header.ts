// Shared panel chrome. Creative lettering belongs to each view's content.
interface PanelHeaderOptions {
  id?: string | null;
  title: string;
  description?: string;
  titleId?: string;
  icon?: string | null;
  collapsible?: boolean;
  actions?: any[] | null;
}

class PanelHeader {
  readonly label: HTMLElement;
  readonly actions: HTMLElement | null;
  readonly element: HTMLElement;

  constructor({id, title, description, titleId, icon = null, collapsible = false, actions = null}: PanelHeaderOptions) {
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
  rename(title: string, description: string = title): void {
    this.label.textContent = title;
    this.element.title = description;
    this.element.setAttribute('aria-label', title);
  }
}

// One label/control shape for inspector forms; hosts choose the control's behavior.
class InspectorField {
  readonly input: any;
  readonly node: HTMLElement;
  constructor(label: string, input: any) {
    this.input=input;
    this.node=el('label',{},el('span',{},label),input);
  }
}

// Arrow, Home and End move between a tablist's tabs the way the workspace bar does.
// Selection follows focus; views that rebuild on select get focus back by label.
const TABLIST_KEYS = ['ArrowLeft', 'ArrowRight', 'Home', 'End'];
function tablistTabs(list: Element): HTMLButtonElement[] {
  return [...list.querySelectorAll<HTMLButtonElement>('[role=tab]')].filter(tab => !tab.disabled);
}
function tablistStep(key: string, index: number, count: number): number {
  if (key === 'Home') return 0;
  if (key === 'End') return count - 1;
  return (index + (key === 'ArrowRight' ? 1 : -1) + count) % count;
}
function tablistSelect(list: Element, next: number): void {
  const label = list.getAttribute('aria-label');
  tablistTabs(list)[next].click();
  const live = list.isConnected || !label ? list : document.querySelector(`[role=tablist][aria-label="${CSS.escape(label)}"]`);
  if (live) tablistTabs(live)[next]?.focus();
}
document.addEventListener('keydown', (event: KeyboardEvent) => {
  if (!TABLIST_KEYS.includes(event.key)) return;
  const tab = (event.target as HTMLElement).closest?.('[role=tab]') as HTMLButtonElement | null;
  const list = tab?.closest('[role=tablist]');
  const tabs = list ? tablistTabs(list) : [];
  const index = tabs.indexOf(tab);
  if (index < 0) return;
  event.preventDefault();
  event.stopPropagation();
  tablistSelect(list, tablistStep(event.key, index, tabs.length));
});
