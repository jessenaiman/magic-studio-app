// Mayhem: code, usage and tool reports. Saved measurements are read on entry;
// scanning is an explicit action, never a timer. Report pieces come from report.js.
const MAYHEM_SECTIONS = [['overview', 'Overview'], ['code', 'Code'], ['usage', 'Usage'], ['tools', 'Tools'],
  ['commands', 'Commands'], ['backstage', 'Backstage'], ['architecture', 'Architecture']];
// Existing Archify references, not live system models. Upstream evidence links stay pinned.
const MAYHEM_DIAGRAMS = [
  {id: 'magic-cli', file: 'magic-cli-architecture.html', title: 'Magic CLI & Studio architecture', source: 'CLI-Anything magic harness · unified live Studio & headless engine routing', revision: null},
  {id: 'creator', file: 'creator-view.html', title: 'Creator’s loop', source: 'Existing Magic Studio creator workflow · saved 27 Oct 2026', revision: null},
  {id: 'pipeline', file: 'pipeline-evidence.html', title: 'Pipeline architecture · upstream evidence', source: 'calesthio/OpenMontage · evidence-linked reference', revision: '3cd5700b977bcd63644a3118cc1783df1cb002c8'},
];
const MAYHEM_SCAN_STATES = {measured: 'Explicit scan', partial: 'Partial scan', saved: 'Saved scan', stale: 'Saved scan · revision changed', worktree_unverified: 'Saved scan · worktree changes not verified'};

function mayhemTime(seconds) { return seconds == null ? 'time unavailable' : new Date(seconds * 1000).toLocaleString(); }
function mayhemMemory(kb) { return kb == null ? '—' : `${reportValue(Number(kb) / 1048576)} GB`; }
function mayhemMeasure(comp, key) { return comp.available === true ? comp[key] : null; }
function mayhemScanLabel(comp) { return comp.available ? MAYHEM_SCAN_STATES[comp.state] || 'Saved measurement' : 'Not measured'; }

function mayhemStats(...stats) { return el('div', {class: 'report-stats'}, ...stats.map((stat) => new ReportStat(stat).element)); }

function mayhemCodeStats(comp) {
  const total = Number(mayhemMeasure(comp, 'functions_analyzed')) || 0, over = Number(mayhemMeasure(comp, 'functions_over_ccn_15')) || 0;
  const top = comp.hotspots?.[0];
  return [
    {label: 'Functions over CCN 15', value: reportValue(mayhemMeasure(comp, 'functions_over_ccn_15')), signal: over ? 'needs-attention' : comp.available ? 'ready' : null,
      share: total ? over / total : null, detail: total ? `${reportValue(100 * over / total)}% of ${reportValue(total)} functions` : comp.reason || 'Scan code to measure.'},
    {label: 'Highest CCN', value: reportValue(mayhemMeasure(comp, 'max_ccn')), detail: top ? `${top.name} · ${top.file}:${top.start_line}` : '—'},
    {label: 'Source lines', value: reportValue(mayhemMeasure(comp, 'total_source_lines'), {compact: true}),
      detail: comp.available ? `${reportValue(comp.files_analyzed)} files with functions` : '—'},
  ];
}

function mayhemUsageStats(usage) {
  const totals = usage.available ? usage.totals || {} : {};
  const active = (usage.agents || []).filter((agent) => agent.today_tokens > 0).length;
  return [
    {label: 'Tokens today', value: reportValue(totals.today_tokens, {compact: true}),
      detail: usage.available ? `${reportValue(totals.today_prompts)} prompts · ${active} agents active` : 'Usage records unavailable.'},
    {label: 'Tokens recorded', value: reportValue(totals.total_tokens, {compact: true}),
      detail: totals.total_tokens ? `${reportValue(100 * totals.cache_read_tokens / totals.total_tokens)}% cache reads` : '—'},
  ];
}

function mayhemToolStat(tools = []) {
  const found = tools.filter((tool) => tool.available).length;
  return {label: 'Tools found', value: tools.length ? `${found} / ${tools.length}` : '—',
    signal: tools.length ? (found === tools.length ? 'ready' : 'needs-attention') : 'idle', detail: 'Executable on PATH · not a run'};
}

const MAYHEM_LANG_LABELS: Record<string, string> = {
  rust: 'Rust', typescript: 'TypeScript', tsx: 'TSX · Remotion', javascript: 'JavaScript',
  css: 'CSS', html: 'HTML · Shell', markdown: 'Markdown', python: 'Python',
};

function inferMayhemLang(file = '', explicit = '') {
  if (explicit) return explicit;
  const ext = file.split('.').pop() || '';
  const map: Record<string, string> = {rs: 'rust', ts: 'typescript', tsx: 'tsx', js: 'javascript', mjs: 'javascript', css: 'css', html: 'html', md: 'markdown', py: 'python'};
  return map[ext] || 'other';
}

function mayhemLangBadge(file = '', explicit = '', customLabel = '') {
  const lang = inferMayhemLang(file, explicit);
  const label = customLabel || MAYHEM_LANG_LABELS[lang] || lang;
  return el('span', {class: 'report-lang', 'data-lang': lang}, label);
}

function mayhemHotspots(comp, limit) {
  return new ReportTable({rows: (comp.hotspots || []).slice(0, limit).map((fn) => ({...fn, lang: inferMayhemLang(fn.file, fn.lang)})), empty: comp.reason || 'No saved scan. Scan code to measure.', columns: [
    {label: 'Lang', value: (fn) => mayhemLangBadge(fn.file, fn.lang)},
    {label: 'Function', value: (fn) => fn.name},
    {label: 'Where', value: (fn) => el('code', {}, `${fn.file.split('/').pop()}:${fn.start_line}`), title: (fn) => `${fn.file}:${fn.start_line}–${fn.end_line}`},
    {label: 'CCN', value: (fn) => fn.ccn, numeric: true, bar: true},
    {label: 'NLOC', value: (fn) => fn.nloc, numeric: true},
  ]}).element;
}

function mayhemLanguageBreakdown(comp) {
  return new ReportTable({rows: comp.languages || [], empty: 'Run Scan code to build the per-language breakdown.', columns: [
    {label: 'Language', value: (row) => mayhemLangBadge('', row.lang, row.label)},
    {label: 'Lines', value: (row) => row.lines, numeric: true, bar: true},
    {label: 'Files', value: (row) => row.files, numeric: true},
    {label: 'Over CCN 15', value: (row) => row.over_15, numeric: true},
    {label: 'Max CCN', value: (row) => row.max_ccn || '—', numeric: true},
    {label: 'Biggest offender', value: (row) => row.top_file ? el('code', {}, `${row.top_file.split('/').pop()} (${reportValue(row.top_file_lines)} lines)`) : '—', title: (row) => row.top_file || null},
  ]}).element;
}

function mayhemAgents(usage, todayOnly) {
  const rows = (usage.agents || []).filter((agent) => (todayOnly ? agent.today_tokens : agent.total_tokens) > 0);
  const limit = (agent) => agent.limits?.[0];
  return new ReportTable({rows: rows, empty: usage.available ? 'No recorded usage.' : 'Usage records unavailable.', columns: [
    {label: 'Agent', value: (agent) => agent.name || agent.id},
    {label: 'Today', value: (agent) => agent.today_tokens, numeric: true, compact: true, bar: true},
    ...(todayOnly ? [] : [
      {label: 'Recorded', value: (agent) => agent.total_tokens, numeric: true, compact: true},
      {label: 'Prompts', value: (agent) => agent.total_prompts, numeric: true},
      {label: 'Top model', value: (agent) => agent.models?.[0]?.model || '—'}]),
    {label: 'Limit used', value: (agent) => limit(agent)?.percent ?? null, numeric: true,
      text: (agent) => limit(agent) ? `${reportValue(100 * limit(agent).percent)}%` : '—', title: (agent) => limit(agent)?.label ?? null},
  ]}).element;
}

function mayhemCoverage(comp) {
  if (!comp.available) return null;
  const failed = comp.scan_failures || [];
  const dirty = comp.source_dirty === true ? 'worktree had changes' : comp.source_dirty === false ? 'clean worktree' : 'worktree unknown';
  return `${mayhemScanLabel(comp)} · ${mayhemTime(comp.measured_at_unix_secs)} · ${dirty}${failed.length ? ` · not measured: ${failed.join(', ')}` : ''} · Lizard ${comp.tool_version || ''}`;
}

function mayhemHealthWarnings(d: any): string[] {
  const comp = d?.complexity, tools = d?.tools;
  const missing = Array.isArray(tools) ? tools.filter(tool => tool.available === false).length : 0;
  const failed = comp?.scan_failures?.length || 0;
  const hotspots = comp?.available === true && Number.isFinite(comp.functions_over_ccn_15) ? comp.functions_over_ccn_15 : 0;
  const warnings = [];
  if (missing) warnings.push(`${missing} tools not found`);
  if (failed) warnings.push(`${failed} scan targets failed`);
  if (hotspots) warnings.push(`${hotspots} functions over CCN 15 in saved scan`);
  if (comp?.last_error || comp?.cache_error) warnings.push('scan error recorded');
  return warnings;
}

class AnalyticsViewController {
  commandFilter = '';
  data: any = null;
  diagram = 'magic-cli';
  error: string | null = null;
  loading = false;
  section = 'overview';

  async render(scan = false) {
    if (!$('view-analytics') || this.loading) return;
    this.mount();
    this.loading = true;
    this.error = null;
    $('analytics-refresh').disabled = true;
    $('analytics-scan').disabled = true;
    $('analytics-message').textContent = scan ? 'Running Lizard on local source…' : 'Reading saved scan and recorded usage…';
    try {
      this.data = await invoke('engine_command', {id: 'app.analytics', params: {scan}});
    } catch (error) {
      this.error = String(error);
    } finally {
      this.loading = false;
      $('analytics-refresh').disabled = false;
      $('analytics-scan').disabled = false;
      this.healthIndicator();
      this.renderSurface();
    }
  }

  mount() {
    if ($('analytics-layout')) return;
    const tabs = el('nav', {class: 'segmented', id: 'mayhem-sections', role: 'tablist', 'aria-label': 'Mayhem sections'},
      ...MAYHEM_SECTIONS.map(([id, label]) => el('button', {id: `analytics-nav-${id}`, role: 'tab',
        onclick: () => { this.section = id; this.renderSurface(); }}, label)));
    const actions = el('div', {class: 'report-actions'},
      el('button', {id: 'analytics-refresh', class: 'ghost', title: 'Read the saved scan and recorded usage without running Lizard', 'data-commands': 'app.analytics', onclick: () => this.render(false)}, 'Refresh'),
      el('button', {id: 'analytics-scan', class: 'start', title: 'Measure local Rust, Python and shell JavaScript with Lizard now', 'data-commands': 'app.analytics', onclick: () => this.render(true)}, 'Scan code'));
    const head = el('header', {class: 'report-head'},
      el('div', {class: 'report-title'}, el('h1', {}, 'Mayhem'),
        el('p', {id: 'analytics-message', class: 'panel-status', role: 'status'}), actions), tabs);
    const surface = el('div', {id: 'analytics-surface', class: 'report-surface'});
    const backstage = el('div', {class: 'report-surface', id: 'analytics-backstage'}, window.magicBackstageTerminal.mount());
    $('view-analytics').replaceChildren(el('div', {class: 'report-page', id: 'analytics-layout'},
      head, el('div', {class: 'report-scroll'}, surface, backstage)));
  }

  renderSurface() {
    const surface = $('analytics-surface');
    if (!surface) return;
    const d = this.data || {};
    const backstage = this.section === 'backstage';
    $('analytics-backstage').hidden = !backstage;
    surface.hidden = backstage;
    if (backstage) window.dispatchEvent(new Event('backstage-visible'));
    for (const [id] of MAYHEM_SECTIONS) $(`analytics-nav-${id}`).setAttribute('aria-selected', String(id === this.section));
    $('analytics-message').textContent = this.error
      ? `Could not refresh: ${this.error}. Previous results, if any, remain below.`
      : `Read ${mayhemTime(d.sampled_at_unix_secs)} · no background scans or polling`;
    surface.replaceChildren(...MAYHEM_RENDER[this.section](d));
  }

  healthIndicator() {
    const warnings = mayhemHealthWarnings(this.data);
    const button = document.getElementById('tab-analytics');
    const stale = Boolean(this.error);
    button.dataset.health = warnings.length ? 'warning' : 'unknown';
    document.getElementById('mayhem-health').textContent = warnings.length ? `${stale ? 'Previous · ' : ''}${warnings.length} signal${warnings.length === 1 ? '' : 's'}` : 'Unknown';
    button.title = `Mayhem · check here first. ${warnings.length ? warnings.join('; ') : 'Overall health unknown; no connected audit result.'}${stale ? ' Refresh failed; previous observations retained.' : ''} Executable presence is not runtime health.`;
  }
}

const mayhem = new AnalyticsViewController();

function mayhemOverview(d) {
  const comp = d.complexity || {}, usage = d.usage || {};
  return [
    mayhemStats(...mayhemCodeStats(comp).slice(0, 2), mayhemUsageStats(usage)[0], mayhemToolStat(d.tools)),
    new ReportSection({title: 'Language & web stack breakdown', note: 'Color-coded by language: Desktop UI uses HTML + TypeScript + CSS; Remotion uses TSX.'}, mayhemLanguageBreakdown(comp)).element,
    el('div', {class: 'report-columns'},
      new ReportSection({title: 'Complexity hotspots', note: mayhemCoverage(comp)}, mayhemHotspots(comp, 6)).element,
      new ReportSection({title: 'Usage today', note: 'Recorded by local agent tools · not billing'}, mayhemAgents(usage, true)).element),
  ];
}

function mayhemFileTable(rows) {
  return new ReportTable({rows: (rows || []).map((f) => ({...f, lang: inferMayhemLang(f.file, f.lang)})), empty: 'No saved scan.', columns: [
    {label: 'Lang', value: (file) => mayhemLangBadge(file.file, file.lang)},
    {label: 'File', value: (file) => el('code', {}, file.file.split('/').pop()), title: (file) => file.file},
    {label: 'Lines', value: (file) => file.lines, numeric: true, bar: true},
    {label: 'Total CCN', value: (file) => file.ccn, numeric: true, bar: true},
    {label: 'Max CCN', value: (file) => file.max_ccn, numeric: true},
    {label: 'Functions', value: (file) => file.functions, numeric: true},
  ]}).element;
}

function mayhemCode(d) {
  const comp = d.complexity || {};
  const provenance = el('details', {class: 'report-provenance'}, el('summary', {}, 'Measurement source and coverage'),
    el('code', {}, `${comp.source_root || 'Source root unavailable'}\nrevision ${comp.source_revision || 'unavailable'}\n${comp.tool_path || 'Tool path unavailable'}\nTargets: ${(comp.targets || []).join(', ')}`));
  return [
    mayhemStats(...mayhemCodeStats(comp)),
    new ReportSection({title: 'By language & biggest offenders', note: 'Physical lines (reading cost) and cyclomatic complexity (change cost) across Rust, TypeScript, CSS, Markdown, HTML, TSX, and Python.'}, mayhemLanguageBreakdown(comp)).element,
    new ReportSection({title: 'Function hotspots', note: 'CCN and NLOC are Lizard measurements; CCN 15 is this repository’s limit.'}, mayhemHotspots(comp, 25)).element,
    el('div', {class: 'report-columns'},
      new ReportSection({title: 'Largest files by line count', note: '2,000 lines is the long-file threshold.'}, mayhemFileTable(comp.longest_files)).element,
      new ReportSection({title: 'Most complex files by total CCN', note: mayhemCoverage(comp)}, mayhemFileTable(comp.largest_ccn_files), provenance).element),
  ];
}

function mayhemUsage(d) {
  const usage = d.usage || {}, project = d.project || {};
  return [
    mayhemStats(...mayhemUsageStats(usage),
      {label: 'This project’s chat', value: project.project ? reportValue(project.estimated_tokens, {compact: true}) : '—',
        detail: project.project ? `${reportValue(project.total_turns)} turns · estimate: characters ÷ 4` : 'No project selected.'}),
    new ReportSection({title: 'Agents', note: 'Omarchy local usage records · not provider billing'}, mayhemAgents(usage, false)).element,
  ];
}

function mayhemTools(d) {
  const host = d.host || {};
  const tools = new ReportTable({rows: d.tools || [], empty: 'Refresh to check tools.', columns: [
    {label: 'Tool', value: (tool) => el('span', {class: 'report-name'}, window.companionBoxes.dot(tool.available ? 'ready' : 'needs-attention'), tool.tool)},
    {label: 'Resolved', value: (tool) => el('code', {}, tool.available ? tool.resolved : 'not found')},
  ]}).element;
  const used = host.mem_total_kb ? host.mem_used_kb / host.mem_total_kb : null;
  return [
    mayhemStats(mayhemToolStat(d.tools),
      {label: 'Studio process', value: mayhemMemory(host.self_rss_kb), detail: `PID ${host.pid ?? '—'} · resident memory`},
      {label: 'System memory', value: mayhemMemory(host.mem_used_kb), share: used, detail: `of ${mayhemMemory(host.mem_total_kb)} · load ${host.load || '—'}`}),
    new ReportSection({title: 'Production tools', note: 'Found on PATH is not a successful run. Read from /proc and PATH.'}, tools).element,
  ];
}

function mayhemCommands(d) {
  const catalog = d.commands;
  if (!catalog?.commands) return [el('p', {class: 'panel-empty', 'data-look': 'inline'}, 'Command metadata unavailable. Refresh to read it.')];
  const input = el('input', {id: 'analytics-command-search', class: 'field report-search', type: 'search', placeholder: 'Find a command or feature',
    'aria-label': 'Find current command', value: mayhem.commandFilter,
    oninput: (event) => { mayhem.commandFilter = event.target.value; mayhemCommandRows(catalog.commands); }});
  const section = new ReportSection({title: `Commands · ${catalog.commands.length}`, note: 'From the shared engine command.list. Run with magic-cli exec ID key=value.'},
    input, el('div', {id: 'analytics-command-list', class: 'report-body'}));
  queueMicrotask(() => mayhemCommandRows(catalog.commands));
  return [section.element];
}

function mayhemCommandRows(commands) {
  const root = $('analytics-command-list');
  if (!root) return;
  const filter = mayhem.commandFilter.toLowerCase();
  const groups = new Map();
  for (const command of commands.filter((c) => `${c.id} ${c.label} ${c.params}`.toLowerCase().includes(filter))) {
    const key = command.menu?.[0] || command.id.split('.')[0];
    groups.set(key, [...(groups.get(key) || []), command]);
  }
  root.replaceChildren(...[...groups].map(([name, items]) => el('details', {class: 'report-group', open: Boolean(filter)},
    el('summary', {}, `${name} · ${items.length}`),
    new ReportTable({rows: items, columns: [
      {label: 'Command', value: (c) => c.label},
      {label: 'ID', value: (c) => el('code', {title: c.params}, c.id)},
      {label: 'Shortcut', value: (c) => c.shortcut || '—'},
    ]}).element)));
}

function mayhemArchitecture() {
  const selected = MAYHEM_DIAGRAMS.find((item) => item.id === mayhem.diagram) || MAYHEM_DIAGRAMS[0];
  const picker = el('select', {id: 'analytics-diagram-select', class: 'field', 'aria-label': 'Architecture reference',
    onchange: (event) => { mayhem.diagram = event.target.value; mayhem.renderSurface(); }},
    ...MAYHEM_DIAGRAMS.map((item) => el('option', {value: item.id}, item.title)));
  picker.value = selected.id;
  return [new ReportSection({title: 'How the work fits together', actions: [picker],
    note: `Saved Archify reference, not a live model · ${selected.source}${selected.revision ? ` · revision ${selected.revision}` : ''}`},
  el('iframe', {id: 'analytics-diagram-frame', class: 'report-frame', title: selected.title,
    src: `analytics-diagrams/${selected.file}?theme=dark&embed=1`, sandbox: 'allow-scripts'})).element];
}

const MAYHEM_RENDER = {overview: mayhemOverview, code: mayhemCode, usage: mayhemUsage, tools: mayhemTools,
  commands: mayhemCommands, backstage: () => [], architecture: mayhemArchitecture};

async function renderAnalytics(scan = false) { return mayhem.render(scan); }
function mayhemMount() { mayhem.mount(); }
function analyticsRenderSurface() { mayhem.renderSurface(); }
function mayhemHealthIndicator() { mayhem.healthIndicator(); }
