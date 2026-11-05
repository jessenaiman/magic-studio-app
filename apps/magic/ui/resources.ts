// Project-independent reference. The CLI and this view edit the same resource.
const workflowCommand = (params = {}) => invoke('engine_command', {id:'resources.workflow', params});
const resourcesRoot = $('view-skills');
let resourcesLoading = false;

async function renderResources() {
  if (resourcesLoading || $('workflow-guide')) return;
  resourcesLoading = true;
  try {
    const {document: guide} = await workflowCommand();
    const status = el('small', {class:'muted', role:'status'}, 'Edit any field. Changes save when you leave it.');
    const output = el('ol', {id:'workflow-terminal-steps'});
    const listSteps = () => output.replaceChildren(...guide.production.map(row => el('li', {}, row.label)));
    listSteps();
    const field = (row, key, label, section = 'production') => {
      const input = el('textarea', {id:`wf-${row.id}-${key}`, rows:1,
        'aria-label':label+' · '+row.label, class:'workflow-field wf-'+key});
      input.value = row[key];
      const resize = () => { input.style.height='auto'; input.style.height=input.scrollHeight+'px'; };
      input.addEventListener('input',resize);
      input.addEventListener('change', async () => {
        const previous = row[key], value = input.value;
        if (value === previous) return;
        input.readOnly = true;
        try {
          await workflowCommand({section,id:row.id,field:key,previous,value});
          row[key]=value; listSteps(); status.textContent='Saved to shared Resources.';
          input.removeAttribute('aria-invalid');
        } catch (e) { input.setAttribute('aria-invalid','true');status.textContent='Unsaved · '+String(e);toast(String(e)); }
        finally {input.readOnly=false;}
      });
      return input;
    };
    const resizeFields = (node: HTMLElement) => node.querySelectorAll('textarea').forEach(input => {
      if (input.clientWidth) {input.style.height='auto';input.style.height=input.scrollHeight+'px';}
    });
    const stages = guide.production.map((row,i) => {
      const body=el('div',{class:'workflow-details'},...Object.entries({
        view:'Magic Studio view',command:'magic-cli',pipeline:'Pipeline',verify:'Verify',
      }).map(([k,label])=>el('label',{},el('small',{},label),field(row,k,label))));
      const more=el('details',{},el('summary',{},'Commands and verification'),body);
      more.open=i===0;
      more.ontoggle=()=>resizeFields(body);
      return el('li',{},el('span',{class:'workflow-number'},String(i+1).padStart(2,'0')),
        el('div',{},field(row,'label','Step'),more));
    });
    const revision=el('details',{class:'workflow-revision'},el('summary',{},'Client revision loop'),
      ...guide.mainPath.map((id,i)=>el('label',{},el('span',{},String(i+1)),
        field(guide.nodes.find(n=>n.id===id),'label','Revision step','revision'))));
    revision.ontoggle=()=>resizeFields(revision);
    const page=el('article',{id:'workflow-guide'},el('header',{},el('small',{},'RESOURCES  /  SHARED GUIDE'),
      el('h1',{},'Music video workflow'),status),el('div',{class:'workflow-columns'},
      el('section',{},el('h2',{},'Production checklist'),el('ol',{class:'workflow-stages'},...stages)),
      el('aside',{},el('h2',{},'Terminal view'),el('code',{},'magic-cli resources workflow steps=true'),output)),revision);
    resourcesRoot.replaceChildren(page);
    requestAnimationFrame(()=>resizeFields(page));
  } catch (e) {resourcesRoot.replaceChildren(el('p',{role:'alert'},String(e)),el('button',{onclick:renderResources},'Retry'));}
  finally {resourcesLoading=false;}
}
