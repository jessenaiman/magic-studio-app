const MAYHEM_LANG_LABELS: Record<string, string> = {
  rust: 'Rust', typescript: 'TypeScript', tsx: 'TSX · Remotion', javascript: 'JavaScript',
  css: 'CSS', html: 'HTML · Shell', markdown: 'Markdown', python: 'Python',
};

function mayhemTime(seconds) { return seconds == null ? 'time unavailable' : new Date(seconds * 1000).toLocaleString(); }
type MayhemScatterRow = {label: string; ccn: number; nloc: number};
type MayhemChartSource = {complexity?: {available?: boolean; languages?: {lang: string; label?: string; lines?: number}[];
  history?: {measured_at_unix_secs?: number; max_ccn?: number}[]};
  usage?: {available?: boolean; agents?: {name?: string; id?: string; today_tokens?: number}[]};
  audit?: {available?: boolean; tools?: {label?: string; tool?: string; findings?: number}[]}};

function mayhemNumber(value: unknown): value is number { return typeof value === 'number' && Number.isFinite(value) && value >= 0; }

function mayhemChartSeries(d: MayhemChartSource) {
  const comp = d.complexity || {}, usage = d.usage || {}, audit = d.audit || {};
  return {
    languages: comp.available ? (comp.languages || []).filter(row => mayhemNumber(row.lines))
      .map(row => ({label: row.label || MAYHEM_LANG_LABELS[row.lang] || row.lang, value: row.lines})) : [],
    usage: usage.available ? (usage.agents || []).filter(row => mayhemNumber(row.today_tokens))
      .map(row => ({label: row.name || row.id || 'Unnamed agent', value: row.today_tokens})) : [],
    history: comp.available ? (comp.history || []).filter(row => mayhemNumber(row.measured_at_unix_secs) && mayhemNumber(row.max_ccn))
      .map(row => ({label: mayhemTime(row.measured_at_unix_secs), value: row.max_ccn})) : [],
    checks: audit.available ? (audit.tools || []).filter(row => mayhemNumber(row.findings))
      .map(row => ({label: row.label || row.tool || 'Unnamed check', value: row.findings})) : [],
  };
}

function mayhemScatterRows(comp: {available?: boolean; hotspots?: {name?: string; file?: string; start_line?: number; ccn?: number; nloc?: number}[]}): MayhemScatterRow[] {
  return comp.available ? (comp.hotspots || []).filter(row => mayhemNumber(row.ccn) && mayhemNumber(row.nloc))
    .map(row => ({label: `${row.name || 'Unnamed'} · ${row.file || '?'}:${row.start_line ?? '?'}`, ccn: row.ccn, nloc: row.nloc})) : [];
}

const mayhemBundles = new Map<string, Promise<void>>();
function mayhemBundle(file: string): Promise<void> {
  // This WebKit UI targets ES2023, before Promise.withResolvers.
  if (!mayhemBundles.has(file)) mayhemBundles.set(file, new Promise((resolve, reject) => {
    const script = document.createElement('script');
    script.src = `vendor/${file}.bundle.js`;
    script.onload = () => resolve();
    script.onerror = () => { script.remove(); mayhemBundles.delete(file); reject(new Error(`Could not load ${file}. Rebuild frontend bundles.`)); };
    document.head.append(script);
  }));
  return mayhemBundles.get(file);
}

function mayhemEvidence(title: string, value: unknown) {
  return el('details', {class: 'mayhem-evidence'}, el('summary', {}, title),
    el('pre', {}, JSON.stringify(value ?? null, null, 2)));
}

// The engine owns the verdict. Opening the pane reads the saved score once.
class AnalyticsViewController {
  data: any = null;
  score: any = null;
  evidence: any = {};
  tree: {dispose():void} | null = null;
  judgeError = '';
  judging = false;
  loading = false;
  chart: {dispose(): void} | null = null;
  detailsOpen = false;

  async runJev() {
    if (this.judging || this.loading) return;
    this.judging = true;
    this.judgeError = null;
    $('analytics-refresh').disabled = true;
    $('analytics-scan').disabled = true;
    this.renderSurface();
    try {
      const result = await invoke('engine_command', {id: 'app.analytics', params: {judge: true}});
      if (!result?.jev || typeof result.jev !== 'object') throw new Error('Backend returned no Jev report.');
      this.data = {...(this.data || {}), jev: result.jev};
    } catch (error) {
      this.judgeError = String(error);
    } finally {
      this.judging = false;
      $('analytics-refresh').disabled = false;
      $('analytics-scan').disabled = false;
      this.renderSurface();
    }
  }

  async render(scan = false) {
    if (this.loading || this.judging) return;
    this.loading = true;
    const host = $('view-analytics');
    if (scan && this.score) this.renderSurface();
    else host.replaceChildren(el('p', {class:'may-summary',role:'status'},'Reading saved evidence…'));
    try {
      this.score = await motionCommand(scan ? 'mayhem.scan' : 'mayhem.score');
      this.evidence = await motionCommand('mayhem.evidence');
      this.loading = false;
      this.data = null;
      this.detailsOpen = false;
      this.renderSurface();
    } catch (e) {
      host.replaceChildren(el('p', {role:'alert'}, String(e)),
        el('button', {onclick:()=>this.render()}, 'Retry'));
    } finally { this.loading = false; }
  }

  clearChart() { this.chart?.dispose(); this.chart = null; this.tree?.dispose();this.tree=null; }

  async trend(node: HTMLElement, history: any[]) {
    const rows = history.filter(h => mayhemNumber(h.lines) && mayhemNumber(h.time))
      .map(h => ({label:mayhemTime(h.time), value:h.lines}));
    if (rows.length < 2) { node.textContent = 'Two scans establish a trend.'; return; }
    try {
      await mayhemBundle('mayhem-charts');
      if (node.isConnected && state.view === 'analytics')
        this.chart = (window as any).magicMayhemCharts.mount(node, rows, 'line', 'var(--accent)', 'lines');
    } catch (e) { node.textContent = String(e); }
  }

  async inspect(panel: HTMLElement) {
    try {
      const report = this.data || await motionCommand('app.analytics');
      if (!panel.isConnected) return;
      this.data = report;
      const series = mayhemChartSeries(report);
      panel.replaceChildren(mayhemEvidence('Code hotspots', mayhemScatterRows(report.complexity)),
        mayhemEvidence('Checks and usage', {checks:series.checks, usage:series.usage}),
        mayhemEvidence('Full native report', report),
        el('button', {onclick:()=>this.runJev()}, 'Run Jev review'));
      if (this.judgeError) panel.append(el('p',{role:'alert'},this.judgeError));
      if (report.jev) panel.append(mayhemEvidence('Jev report', report.jev));
    } catch (e) { panel.textContent = String(e); }
  }

  renderSurface() {
    this.clearChart();
    const d = this.score || {}, sig = d.signal || 'yellow';
    const title = {red:'Rule broken',yellow:'Needs review',green:'Checks passed'}[sig];
    const indicator = window.companionBoxes.dot({red:'failed',yellow:'needs-attention',green:'ready'}[sig]);
    const actions = el('div',{class:'report-actions'},
      el('button',{id:'analytics-refresh',class:'ghost',onclick:()=>this.render()},'Refresh'),
      el('span',{id:'analytics-scan',hidden:true}));
    const stamp = d.measured_at ? 'Saved scan · '+mayhemTime(d.measured_at)+' · '+String(d.revision || '').slice(0,7) : 'No saved scan';
    const review = d.package_review;
    const brief = ['Mayhem rule #1: Could a maintained package replace this custom logic?',
      'Review measured snapshot '+d.snapshot+'. Start at '+(d.hotspot?.file || 'the changed source')+'.',
      'Compare an actual package with the implementation. Cite the code and package documentation, then recommend keeping, simplifying or replacing it.',
      'Record the conclusion using magic-cli commands mayhem.review.'].join('\n');
    const trend = el('div',{class:'may-trend',role:'img','aria-label':'Source lines across saved scans'});
    const advancedBody = el('div', {});
    const advanced = el('details',{class:'may-more'}, el('summary',{},'Diagnostics'),advancedBody);
    advanced.open = this.detailsOpen;
    advanced.ontoggle = () => { this.detailsOpen = advanced.open; if (advanced.open && !advancedBody.childNodes.length) this.inspect(advancedBody); };
    const terminal = el('details',{class:'may-more'},el('summary',{},'Backstage terminal'));
    terminal.ontoggle = () => {
      if (terminal.open) { terminal.append(window.magicBackstageTerminal.mount()); window.dispatchEvent(new Event('backstage-visible')); }
    };
    const e=this.evidence, agents=e.agents, functions=e.critical_functions, files=e.oversized_files || [];
    const measured=Array.isArray(functions), violations=measured ? functions.length+files.length : null;
    const scanButton=el('button',{id:'mayhem-search',class:'may-search',disabled:this.loading,
      'data-signal':this.loading?'yellow':violations?'red':'green',onclick:()=>this.render(true)},
      this.loading?'Scanning…':violations ? `Scan again · ${violations} violations` : 'Search for violations');
    const part=(n:string,name:string,...body:any[])=>el('section',{class:'may-part'},
      el('h2',{},el('small',{},n),name),...body);
    const nums=el('div',{class:'may-numbers'},...(d.metrics || []).map(m=>el('div',{},
      el('span',{},m.label),el('strong',{},m.current?.toLocaleString() ?? '—'),
      el('small',{},m.delta==null?'No baseline':m.delta===0?'No change':(m.delta>0?'+':'−')+Math.abs(m.delta).toLocaleString()))));
    const traces=(items:any[],format:(row:any)=>string)=>el('ul',{class:'may-list'},
      ...items.map(row=>el('li',{},format(row))));
    const offenders=[...(functions || []),...files];
    const finding=(f:any)=> f.name ? `${f.name} · ${f.file}:${f.start_line} · CCN ${f.ccn}, ${f.nloc} code lines` :
      `${f.file} · ${f.lines} lines`;
    const cli=e.cli_scope, count=cli?.functions||0, usage=e.cli_events||[];
    const hierarchy=el('details',{class:'may-more'},el('summary',{},'Explore magic-cli commands'));
    hierarchy.ontoggle=async()=>{
      if (!hierarchy.open) {this.tree?.dispose();this.tree=null;hierarchy.querySelector('.may-tree')?.remove();return;}
      const node=el('div',{class:'may-tree','aria-label':'Command groups and registered commands'});hierarchy.append(node);
      try {await mayhemBundle('mayhem-charts');if(node.isConnected) this.tree=(window as any).magicMayhemTree(node,e.commands||[]);}
      catch(err){node.textContent=String(err);}
    };
    const tools=['lizard','eslint','stylelint','tsc','clippy'];
    const scanners=tools.map(id=>{
      const t=(e.scanners || []).find(t=>t.tool===id);
      const hits=t?.issues || [];
      return el('article',{class:'may-tool'},el('strong',{},t?.label || id),
        el('span',{},id==='lizard' && measured ? `${violations} current violations` :
          t ? `${t.status} · ${t.findings} findings` : 'No saved check'),
        el('p',{},id==='lizard' && offenders.length ? finding(offenders[0]) :
          hits.length ? `${hits[0].filePath}:${hits[0].line} · ${hits[0].message}` :
          t?.findings?'This older report contains counts only.':t?.status==='passed'?'No finding in this check.':'No issue recorded.'),
        el('details',{},el('summary',{},'Trace and findings'),el('code',{},`magic-cli audit --tool ${id}`),
          el('p',{},t?.measured_at ? mayhemTime(t.measured_at) : 'See saved audit revision '+String(e.audit_revision||'unknown').slice(0,7)),
          id==='lizard'?traces(offenders,finding):traces(hits,r=>`${r.filePath}:${r.line} · ${r.patternId} · ${r.message}`),
          el('pre',{},t?.trace || t?.error || 'Raw output was not retained for this run.')));
    });
    const trust=d.snapshot ? d.revision_changed?'Earlier revision':d.working_tree_modified?'Working-tree snapshot':'Recorded revision':'Unmeasured';
    const content=el('div',{class:'may-summary','data-signal':sig},
      el('header',{class:'may-head'},el('div',{},el('small',{},'MAYHEM  /  RULE 01'),el('h1',{},'Am I good?')),actions),
      part('01','Is Mayhem trustworthy?',el('strong',{},trust),el('small',{class:'may-stamp'},stamp),
        el('p',{},'Could a maintained package replace this logic? ', review ? [review.verdict,review.finding,review.reviewer,review.evidence].join(' · '):'Outside review pending.'),
        d.snapshot?el('button',{class:'ghost',onclick:async()=>{try{await navigator.clipboard.writeText(brief);toast('Review brief copied');}catch(err){toast(String(err));}}},'Copy review brief'):null),
      part('02','Where are my agents?',Array.isArray(agents) && agents.length ?
        traces(agents,a=>`${a.agent} · ${a.agent_status||'status unknown'} · ${a.cwd||'location unknown'} · ${a.pane_id||''}`):
        el('p',{},Array.isArray(agents)?'Herdr reports no agents.':'Agent inventory unavailable.')),
      part('03','God functions and oversized files',scanButton,el('strong',{},indicator,' ',title),el('p',{},d.reason),nums,
        traces(offenders.slice(0,3),finding),offenders.length>3?el('details',{},el('summary',{},'All violations'),traces(offenders,finding)):null,
        measured?null:el('p',{},'Run a source scan for the full list.'),el('details',{},el('summary',{},'Source line history'),trend)),
      part('04','magic-cli',el('p',{},count?`${Math.round(100*cli.within_limits/count)}% of measured CLI functions meet the branching and length limits · ${cli.within_limits}/${count} · ${cli.source_lines} source lines`:'CLI size and standards coverage need a scan.'),
        el('p',{},usage.length?`Observed CLI call: ${usage[usage.length-1].command}`:'No CLI use observed in the retained events for this context.'),hierarchy),
      part('05','Scanner findings',...scanners),advanced,terminal);
    $('view-analytics').replaceChildren(content);
    const nav = $('mayhem-health');
    if (nav) { nav.textContent = title; nav.parentElement.dataset.health = sig; }
    const historyBox=trend.parentElement as HTMLDetailsElement;
    historyBox.ontoggle=()=>{if(historyBox.open && !this.chart) void this.trend(trend,d.history || []);};
  }
}
const mayhemView = new AnalyticsViewController();
async function renderAnalytics(scan = false) { return mayhemView.render(scan); }
