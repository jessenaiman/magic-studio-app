// Reusable presentation for button menus and text context menus in every workspace.
// Labels, shortcuts and action IDs come from the native catalog; no per-view copies.
class StudioMenu {
  items: any;
  kind: any;
  range: any;
  root: any;
  selection: any;
  supported: any;
  target: any;
  trigger: any;
  static active = null;
  constructor(kind, trigger = null) {
    this.kind = kind;
    this.trigger = trigger;
    this.items = [];
    this.root = el('div', {id:`${kind}-menu`, class:'popup-menu', role:'menu', hidden:true});
    document.body.append(this.root);
    trigger?.addEventListener('click', () => {
      if (StudioMenu.active === this) return this.close();
      const rect = trigger.getBoundingClientRect();
      this.open(rect.left, rect.bottom + 4, trigger);
    });
    trigger?.addEventListener('keydown', event => {
      if (event.key !== 'ArrowDown') return;
      event.preventDefault();
      trigger.click();
    });
    this.root.addEventListener('keydown', event => this.onKey(event));
    this.root.addEventListener('pointerdown', event => event.preventDefault());
    this.root.addEventListener('click', event => event.stopPropagation());
    document.addEventListener('pointerdown', event => {
      if (StudioMenu.active === this && !this.root.contains(event.target) && !trigger?.contains(event.target)) this.close(false);
    });
    window.addEventListener('resize', () => this.close(false));
  }
  configure({label, items, supported = true}) {
    this.items = items;
    this.supported = supported;
    this.root.setAttribute('aria-label', label);
    if (this.trigger) {
      this.trigger.firstChild.textContent = label;
      this.trigger.setAttribute('aria-label', `${label} menu`);
      this.trigger.setAttribute('aria-controls', this.root.id);
      this.trigger.setAttribute('aria-expanded', 'false');
    }
  }
  open(x, y, target = this.trigger) {
    if (this.supported === false) throw new Error("Use the platform’s native text context menu");
    if (!Number.isFinite(x) || !Number.isFinite(y)) throw new Error("Menu position must be a number");
    StudioMenu.active?.close(false);
    this.target = target;
    this.selection = target && typeof target.selectionStart === 'number'
      ? [target.selectionStart, target.selectionEnd, target.selectionDirection] : null;
    const selection = window.getSelection();
    this.range = target?.isContentEditable && selection.rangeCount ? selection.getRangeAt(0).cloneRange() : null;
    this.root.replaceChildren(...this.items.flatMap(item => [
      item.separator ? el('div', {role:'separator'}) : null,
      el('button', {id:`menu-${item.id.replaceAll('.', '-')}`, role:'menuitem', tabindex:-1,
        disabled:item.project && !state.project, 'data-menu-action':item.id,
        onclick:()=>this.activate(item.id)},
        el('span', {}, item.label), item.shortcut ? el('kbd', {}, item.shortcut.replace('CmdOrCtrl+', navigator.platform.includes('Mac') ? '⌘' : 'Ctrl+')) : null)
    ]).filter(Boolean));
    this.root.hidden = false;
    const rect = this.root.getBoundingClientRect();
    this.root.style.setProperty('--menu-x', `${Math.max(8, Math.min(x, innerWidth - rect.width - 8))}px`);
    this.root.style.setProperty('--menu-y', `${Math.max(8, Math.min(y, innerHeight - rect.height - 8))}px`);
    this.trigger?.setAttribute('aria-expanded', 'true');
    StudioMenu.active = this;
    this.root.querySelector('button:not(:disabled)')?.focus({preventScroll:true});
  }
  close(restore = true) {
    if (StudioMenu.active !== this) return;
    this.root.hidden = true;
    this.trigger?.setAttribute('aria-expanded', 'false');
    StudioMenu.active = null;
    if (restore) this.restoreTarget();
  }
  restoreTarget() {
    if (!this.target?.isConnected) return;
    this.target.focus({preventScroll:true});
    if (this.selection) this.target.setSelectionRange(...this.selection);
    if (this.range) { const selection = window.getSelection(); selection.removeAllRanges(); selection.addRange(this.range); }
  }
  async activate(action) {
    this.close();
    if (action.startsWith('file.') || action === 'app.settings') return window.studioMenu(action);
    // Closing must not count its own IPC as pending editing work. The native
    // CloseRequested handler still checks every real pending operation and draft.
    try { await (action === 'app.quit' ? nativeInvoke : invoke)('menu_action', {action}); }
    catch (error) { toast(String(error)); }
  }
  onKey(event) {
    if (!['ArrowDown','ArrowUp','Home','End','Escape','Tab'].includes(event.key)) return;
    event.preventDefault();event.stopPropagation();
    if (['Escape','Tab'].includes(event.key)) return this.close();
    const buttons = [...this.root.querySelectorAll('button:not(:disabled)')];
    const index = buttons.indexOf(document.activeElement);
    const next = event.key === 'Home' ? 0 : event.key === 'End' ? buttons.length - 1
      : (index + (event.key === 'ArrowDown' ? 1 : -1) + buttons.length) % buttons.length;
    buttons[next]?.focus();
  }
  attach(root, selector) {
    const open = (event, keyboard = false) => {
      const target = event.target.closest(selector);
      if (!target || target.disabled || this.supported === false) return;
      event.preventDefault();
      target.focus({preventScroll:true});
      const rect = target.getBoundingClientRect();
      this.open(keyboard ? rect.left : event.clientX, keyboard ? rect.bottom : event.clientY, target);
    };
    root.addEventListener('contextmenu', event => open(event));
    root.addEventListener('keydown', event => {
      if (event.key === 'ContextMenu' || (event.shiftKey && event.key === 'F10')) open(event, true);
    });
  }
}

class MenuBarController {
  dialog: any;
  fileMenu: StudioMenu;
  projectSavePending = false;
  textMenu: StudioMenu;

  constructor() {
    this.fileMenu = new StudioMenu('file', $('app-file'));
    this.textMenu = new StudioMenu('text');
    this.textMenu.attach(document, 'input:not([type=range]):not([type=checkbox]), textarea, [contenteditable=true]');
    this.dialog = this.createSettingsDialog();
    document.body.append(this.dialog);
    $('application-settings-form').addEventListener('submit', event => this.submitSettings(event));
  }

  createSettingsDialog() {
    return el('dialog', {id:'application-settings', 'aria-labelledby':'application-settings-title'},
      el('div', {class:'panel-head'}, el('h2', {id:'application-settings-title'}, 'Application Settings'),
        el('button', {class:'icon', title:'Close Settings', 'aria-label':'Close Settings', onclick:()=>this.dialog.close()}, '×')),
      el('form', {id:'application-settings-form'},
        el('label', {}, 'Production project root', el('input', {id:'application-settings-root', required:'', placeholder:'/absolute/path', 'aria-label':'Production project root'})),
        el('button', {type:'button', class:'ghost', onclick:()=>this.chooseRootFolder()}, 'Choose folder…'),
        el('p', {class:'why'}, 'Default location for new projects. Existing projects and one-project overrides are unchanged. Agent/model settings remain in Fidget.'),
        el('p', {id:'application-settings-status',role:'status'}),
        el('button', {id:'application-settings-save',type:'submit',class:'start'}, 'Save Settings')));
  }

  async chooseRootFolder() {
    try {
      const path = await dialog.open({title:'Production project root',directory:true,defaultPath:$('application-settings-root').value||undefined});
      if (path) $('application-settings-root').value = path;
    } catch (error) {
      $('application-settings-status').textContent = String(error);
    }
  }

  async showApplicationSettings() {
    // The video child webview sits above DOM content; return through the existing view guard.
    if ((state.view==='studio'||document.body.classList.contains('split')) && await setView('project')===false) return;
    this.dialog.showModal();
    $('application-settings-save').disabled=true;
    $('application-settings-status').textContent='Loading settings…';
    try {
      const settings=await invoke('engine_command',{id:'project.root',params:{}});
      $('application-settings-root').value=$('application-settings-root').defaultValue=settings.root;
      $('application-settings-status').textContent=`${settings.source} · ${settings.config}`;
      $('application-settings-save').disabled=false;
      $('application-settings-root').focus();
    } catch(error){$('application-settings-status').textContent=String(error);}
  }

  async submitSettings(event) {
    event.preventDefault();
    $('application-settings-save').disabled=true;
    try {
      const settings=await invoke('engine_command',{id:'project.root',params:{root:$('application-settings-root').value}});
      $('application-settings-root').value=$('application-settings-root').defaultValue=settings.root;
      $('application-settings-status').textContent=`Saved · ${settings.config}`;
    } catch(error){$('application-settings-status').textContent=String(error);}
    finally{$('application-settings-save').disabled=false;}
  }

  async saveProjectFromMenu(saveAs=false){
    if(this.projectSavePending)return;
    const project=state.project;
    if(!project){toast('Open or create a project before saving');return;}
    this.projectSavePending=true;
    const wasInert=document.body.inert;
    try {
      // Freeze user editing until identity has changed or the request has failed. The native
      // file picker remains usable; blur first commits the currently focused storyboard field.
      if(saveAs){document.body.inert=true;document.activeElement?.blur();}
      const path=saveAs ? await dialog.save({title:'Save Project As',defaultPath:project,
        filters:[{name:'Magic Studio project',extensions:['magic']}]}) : null;
      if(saveAs && !path)return;
      await this.commitProjectSave(project, saveAs, path);
      toast('Project saved');
    } finally {if(saveAs)document.body.inert=wasInert;this.projectSavePending=false;}
  }

  async commitProjectSave(project, saveAs, path) {
    if(state.project!==project)throw Error('The open project changed; save was not submitted');
    if(projectView.textDrafts.size || state.data?.design?.dirty)await saveStoryboard();
    if(state.project!==project)throw Error('The open project changed; save was not submitted');
    if(saveAs && typeof theatreGuardIdentityChange==='function')await theatreGuardIdentityChange(project);
    await designCommand('project.save',{expected_project:project,...(saveAs?{path}:{})});
  }

  async dispatch(id) {
    try {
      if(this.projectSavePending){toast('Wait for the current save to finish');return;}
      switch(id){
        case 'file.new': await beginNewProject(); break;
        case 'file.open': await openDialog(); break;
        case 'file.save': await this.saveProjectFromMenu(); break;
        case 'file.save_as': await this.saveProjectFromMenu(true); break;
        case 'file.close': await showHome(); break;
        case 'app.settings': await this.showApplicationSettings(); break;
      }
    } catch(error){toast(String(error));}
  }
}

const menuBarController = new MenuBarController();
window.applicationFileMenu = menuBarController.fileMenu;
window.textContextMenu = menuBarController.textMenu;
const applicationSettings = menuBarController.dialog;

async function showApplicationSettings() {
  return menuBarController.showApplicationSettings();
}
async function saveProjectFromMenu(saveAs=false) {
  return menuBarController.saveProjectFromMenu(saveAs);
}
window.studioMenu = id => menuBarController.dispatch(id);
