// Native tools own findings. This runner only records commands, raw output and exit status.
import {spawnSync} from 'node:child_process';
import {mkdirSync, openSync, closeSync, readFileSync, writeFileSync, renameSync, existsSync, statSync} from 'node:fs';
import {join, relative, extname} from 'node:path';

const commands = [
  ['clippy', 'cargo clippy --quiet --workspace --lib --bins --locked --offline --message-format=short --color never -- -D warnings'],
  ['tsc', 'tsc -p . --pretty false'],
  ['eslint', 'eslint --format stylish'],
  ['stylelint', 'stylelint --formatter string'],
  ['lizard', 'lizard --xml -C 15 -L 100 -x */node_modules/* -x */target/* -x */vendor/* -x */apps/src-tauri/ui/*.js'],
  ['wallace', 'wallace'],
  ['jscpd', 'jscpd --reporters console,json --output'],
  ['purgecss', 'purgecss -c purgecss.config.cjs'],
  ['scope', 'git diff --stat=160'],
];
const {root, target = root, quick = false, tools = [], base = 'HEAD', dry_run = false} = JSON.parse(process.argv[2]);
const defaults = {'.rs': ['clippy'], '.css': ['stylelint'], '.ts': ['tsc', 'eslint'], '.js': ['tsc', 'eslint'], '.mjs': ['tsc', 'eslint']};
const selected = tools.length ? tools : quick ? defaults[extname(target)] || ['tsc', 'eslint', 'stylelint'] : commands.map(([name]) => name);
if (selected.some(name => !commands.some(([known]) => name === known))) throw Error('Unknown analyzer selection');
const folder = join(root, 'target/analytics/analyzer-runs', `${Date.now()}-${process.pid}`);
if (!dry_run) mkdirSync(folder, {recursive: true});
const reportFile = path => {
  try { return {path, contents: readFileSync(path, 'utf8')}; }
  catch (error) { return {path, error: error.message}; }
};
const outcomes = [];
for (const [tool, command] of commands.filter(([name]) => selected.includes(name))) {
  // Only fixed definitions are split. User paths are separate argv entries, never shell code.
  const [program, ...args] = command.split(' ');
  const directory = statSync(target).isDirectory();
  const scope = ['clippy', 'tsc', 'purgecss'].includes(tool) ? root : tool === 'wallace' && directory ? join(root, 'apps/src-tauri/ui/style.css') : target;
  if (tool === 'scope') args.push(base, '--', target);
  else if (tool === 'jscpd') args.push(join(folder, 'jscpd'), target);
  else if (tool === 'stylelint') args.push(directory ? join(target, '**/*.css') : target);
  else if (tool === 'wallace') args.push(relative(root, scope));
  else if (!['clippy', 'tsc', 'purgecss'].includes(tool)) args.push(target);
  const local = join(root, 'node_modules/.bin', program);
  const executable = existsSync(local) ? local : program;
  const row = {tool, cwd: root, scope, executable, args, exit_code: null, signal: null, error: null, stdout: null, stderr: null};
  if (!dry_run) {
    const stdout = join(folder, `${tool}.stdout`), stderr = join(folder, `${tool}.stderr`);
    const out = openSync(stdout, 'w'), err = openSync(stderr, 'w');
    let result;
    try { result = spawnSync(executable, args, {cwd: root, stdio: ['ignore', out, err]}); }
    finally { closeSync(out); closeSync(err); }
    Object.assign(row, {exit_code: result.status, signal: result.signal, error: result.error?.message || null,
      stdout: result.error ? null : readFileSync(stdout, 'utf8'), stderr: result.error ? null : readFileSync(stderr, 'utf8')});
    if (tool === 'jscpd') row.native_report = reportFile(join(folder, 'jscpd/jscpd-report.json'));
  }
  outcomes.push(row);
}
const git = args => {
  const result = spawnSync('git', args, {cwd: root, encoding: 'utf8'});
  return result.status === 0 ? result.stdout.trim() : null;
};
const failed = outcomes.find(row => row.exit_code !== 0);
const report = {schema: 'magic.analyzers/1', source_root: root, target, base,
  profile: tools.length ? 'selected' : quick ? 'quick' : 'comprehensive', dry_run,
  revision: git(['rev-parse', 'HEAD']), working_tree: git(['status', '--porcelain']),
  measured_at: Math.floor(Date.now() / 1000), artifacts: dry_run ? null : folder, tools: outcomes,
  exit_code: dry_run ? 0 : failed ? failed.exit_code ?? 2 : 0};
if (!dry_run) {
  const contents = JSON.stringify(report, null, 2);
  writeFileSync(join(folder, 'report.json'), contents);
  writeFileSync(join(folder, 'latest.json'), contents);
  renameSync(join(folder, 'latest.json'), join(root, 'target/analytics/analyzers.json'));
}
process.stdout.write(JSON.stringify(report));
