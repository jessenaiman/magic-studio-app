// One shell for chat, artwork and the read-only lyrics reader. Contents never remount on toggle.
interface TopDrawerOptions {
  root?: Document;
  canOpen?: () => boolean;
  onChange?: (name: string | null, previous?: string | null) => void;
  onLayout?: (height: number) => void;
  onSettings?: (id: string) => void;
}

class TopDrawersController {
  active: string | null = null;
  canOpen: () => boolean;
  host: any;
  lastHeight = -1;
  onChange: (name: string | null, previous?: string | null) => void;
  onLayout: (height: number) => void;
  onSettings: (id: string) => void;
  root: Document;

  constructor({root = document, canOpen = () => true, onChange = () => {}, onLayout = () => {}, onSettings = () => {}}: TopDrawerOptions = {}) {
    this.root = root;
    this.canOpen = canOpen;
    this.onChange = onChange;
    this.onLayout = onLayout;
    this.onSettings = onSettings;
    this.host = root.getElementById('magic-workstation');
    this.bindEvents();
    const observer = new ResizeObserver(() => this.layout());
    observer.observe(this.host);
    window.addEventListener('resize', () => this.layout());
    this.layout();
  }

  panels() {
    return [...this.root.querySelectorAll('[data-drawer]')] as HTMLElement[];
  }

  buttons() {
    return [...this.root.querySelectorAll('[data-drawer-toggle]')] as HTMLElement[];
  }

  layout = () => {
    const height = this.active ? this.host.getBoundingClientRect().height : 0;
    if (height !== this.lastHeight) {
      this.lastHeight = height;
      this.onLayout(height);
    }
  };

  show(name: string | null, restoreFocus = false) {
    const panel = this.panels().find(node => node.dataset.drawer === name);
    if (name && (!panel || !this.canOpen())) return false;
    if (name === this.active) return true;
    const previous = this.active;
    this.active = name;
    for (const node of this.panels()) {
      node.hidden = node !== panel;
      node.setAttribute('aria-hidden', String(node !== panel));
    }
    for (const button of this.buttons()) {
      const open = button.dataset.drawerToggle === name;
      button.setAttribute('aria-expanded', String(open));
      button.setAttribute('aria-pressed', String(open));
    }
    this.host.hidden = !name;
    this.onChange(name, previous);
    this.layout();
    if (restoreFocus) this.buttons().find(node => node.dataset.drawerToggle === previous)?.focus();
    return true;
  }

  open = (name: string | null) => this.show(name);
  close = (restoreFocus = true) => this.show(null, restoreFocus);
  toggle = (name: string) => this.show(name === this.active ? null : name);

  bindEvents() {
    this.root.addEventListener('click', event => {
      const target = event.target as HTMLElement;
      const toggle = target.closest('[data-drawer-toggle]') as HTMLElement | null;
      if (toggle) { this.toggle(toggle.dataset.drawerToggle); return; }
      const settings = target.closest('[data-drawer-settings]') as HTMLElement | null;
      if (settings) {
        const node = this.root.getElementById(settings.dataset.drawerSettings);
        if (node) {
          node.hidden = !node.hidden;
          settings.setAttribute('aria-expanded', String(!node.hidden));
          if (!node.hidden) this.onSettings(node.id);
        }
        return;
      }
      if (target.closest('[data-drawer-close]')) { this.close(); return; }
      if (this.active && !this.host.contains(target)) this.close(false);
    });
    this.root.addEventListener('keydown', event => {
      if (event.key === 'Escape' && this.active && !this.root.getElementById('point-overlay')?.classList.contains('on')) {
        event.preventDefault();
        this.close();
      }
    });
  }
}

function createTopDrawers(options: TopDrawerOptions = {}) {
  return new TopDrawersController(options);
}
