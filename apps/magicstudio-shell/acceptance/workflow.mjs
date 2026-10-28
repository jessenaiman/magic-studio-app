// Infrastructure only: no workflow is approved by the presence of this helper.
import { execFile, execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { appendFile, cp, mkdir, readFile, realpath, rm } from 'node:fs/promises';
import { basename, isAbsolute, join, relative, resolve } from 'node:path';
import assert from 'node:assert/strict';

const inside = (root, path) => {
  const result = resolve(path);
  const rel = relative(root, result);
  assert(rel !== '..' && !rel.startsWith('../') && !isAbsolute(rel), 'Path must remain in disposable workspace');
  return result;
};
const requirement = (value, name) => {
  assert(value && typeof value.description === 'string' && value.description.trim(), `${name} needs a readable expectation`);
  assert.equal(typeof value.check, 'function', `${name} needs an assertion callback`);
  return value;
};

/** Call from an approved WDIO spec; assertion callbacks must throw on mismatch.
 * Approval must cite Jesse's actual observation/approval, never the issue/spec itself.
 * The disposable project lives inside this run's artifact directory and is removed by
 * `cleanup()`; only hashes, reports and small screenshots remain. `fixture` copies
 * selected entries of a real project (never modified) into that disposable folder.
 */
export async function createWorkflow({ browser, cliPath = process.env.MAGIC_ACCEPTANCE_CLI, approval, artifactDir = process.env.MAGIC_ACCEPTANCE_ARTIFACTS, fixture }) {
  assert(approval?.reference && approval.approvedBy && approval.approvedAt, 'Human UI approval evidence is required');
  assert(isAbsolute(cliPath), 'Use the verified installed magic-cli absolute path');
  assert(isAbsolute(artifactDir), 'Use the launcher-owned artifact directory');
  const root = join(artifactDir, 'project');
  await mkdir(root, { recursive: true });
  if (fixture) {
    assert(isAbsolute(fixture.source) && fixture.project && Array.isArray(fixture.include), 'Fixture needs source, project and include');
    for (const entry of [fixture.project, ...fixture.include]) {
      await cp(join(fixture.source, entry), join(root, entry), { recursive: true, errorOnExist: true, force: false });
    }
  }
  const project = join(root, fixture ? basename(fixture.project) : 'Acceptance.magic');
  const sessionFile = join(root, 'session.json');
  await mkdir(artifactDir, { recursive: true });
  const stream = join(artifactDir, 'steps.jsonl');
  let sequence = 0;
  let blocked = false;
  const record = async (entry) => appendFile(stream, `${JSON.stringify({ timestamp: new Date().toISOString(), approval, root, project, sessionFile, ...entry })}\n`);
  await record({ status: 'pending', action: 'workflow', reason: 'No steps have completed' });

  async function fileEvidence(path) {
    const selected = inside(root, path);
    inside(root, await realpath(selected)); // Refuse symlinks escaping the disposable project.
    const bytes = await readFile(selected);
    return { path: selected, bytes: bytes.length, sha256: createHash('sha256').update(bytes).digest('hex'), content: bytes.toString('utf8') };
  }

  async function step({ name, action, expectations, perform }) {
    assert(!blocked, 'An unknown outcome blocked this workflow; inspect evidence before a new run');
    assert(name?.trim(), 'Step needs a name');
    const id = ++sequence;
    const entry = { id, name, action, startedAt: new Date().toISOString(), status: 'pending', expectations: Object.fromEntries(Object.entries(expectations).map(([key, value]) => [key, value.description])) };
    await record(entry);
    try {
      entry.actual = await perform();
      if (entry.actual.unknownOutcome) {
        blocked = true;
        throw new Error('CLI timed out or lost its reply: outcome unknown, no automatic retry');
      }
      const full = join(artifactDir, `step-${id}.png`);
      await browser.saveScreenshot(full);
      // Keep a small preview only; rerun the case if more detail is needed.
      entry.screenshot = join(artifactDir, `step-${id}.jpg`);
      execFileSync('magick', [full, '-resize', '640x>', '-quality', '60', entry.screenshot]);
      await rm(full);
      entry.observations = {};
      for (const [key, expectation] of Object.entries(expectations)) {
        const result = await expectation.check(entry.actual, { browser, root, project, sessionFile, fileEvidence });
        assert.notEqual(result, false, 'Expectation returned false');
        entry.observations[key] = result ?? 'assertions completed';
      }
      entry.status = 'pass';
    } catch (error) {
      entry.status = blocked ? 'blocked' : 'fail';
      entry.error = String(error.stack || error);
      throw error;
    } finally {
      entry.finishedAt = new Date().toISOString();
      await record(entry);
    }
    return entry.actual;
  }

  async function cli({ name, args, targetProject = project, initializeProject = false, modifiesFiles = false, expectExit, expectResult, expectIdentity, expectUI, expectFiles, timeout = 60000 }) {
    assert(Array.isArray(args) && args.every((arg) => typeof arg === 'string'), 'CLI args must be a string array');
    assert(!args.some((arg) => /^--(project|session-file|bridge)(=|$)/.test(arg)), 'Routing is owned by the workflow');
    const selected = inside(root, targetProject);
    if (initializeProject) assert.deepEqual(args, ['project', 'new', `path=${selected}`], 'Only explicit disposable project creation may omit initial --project selection');
    if (modifiesFiles || initializeProject) requirement(expectFiles, 'saved file state');
    const argv = ['--json', '--session-file', sessionFile, ...(initializeProject ? [] : ['--project', selected]), ...args];
    const expectations = {
      exit: requirement(expectExit, 'exit status'), result: requirement(expectResult, 'structured result'),
      identity: requirement(expectIdentity, 'project/window identity'), ui: requirement(expectUI, 'visible UI'),
      ...(expectFiles ? { files: requirement(expectFiles, 'saved file state') } : {}),
    };
    return step({ name, action: { kind: 'magic-cli', executable: cliPath, args: argv, targetProject: selected }, expectations,
      perform: () => new Promise((done) => {
        // execFile owns just this child. Killing it does not prove the native engine cancelled.
        execFile(cliPath, argv, { cwd: root, env: { ...process.env, MAGIC_SESSION_FILE: sessionFile }, timeout, maxBuffer: 16 * 1024 * 1024 }, (error, stdout, stderr) => {
          let result;
          let parseError;
          try { result = JSON.parse(stdout); } catch (failure) { parseError = String(failure); }
          done({ exitCode: error ? (typeof error.code === 'number' ? error.code : null) : 0, stdout, stderr, result, parseError,
            signal: error?.signal, error: error ? String(error) : undefined,
            unknownOutcome: Boolean(error?.killed || error?.signal || (error && typeof error.code !== 'number')) });
        });
      }),
    });
  }

  async function ui({ name, action, modifiesFiles = false, expectIdentity, expectUI, expectFiles }) {
    if (modifiesFiles) requirement(expectFiles, 'saved file state');
    requirement(action, 'UI action');
    return step({ name, action: { kind: 'native-ui', description: action.description }, expectations: {
      identity: requirement(expectIdentity, 'project/window identity'), ui: requirement(expectUI, 'visible UI'),
      ...(expectFiles ? { files: requirement(expectFiles, 'saved file state') } : {}),
    }, perform: async () => ({ observed: await action.check(browser, { root, project, sessionFile, fileEvidence }) }) });
  }

  return { root, project, sessionFile, cli, ui, fileEvidence,
    pending: async (name, reason) => record({ name, status: 'pending', reason }),
    // Remove the disposable project copy; reports, hashes and previews stay.
    cleanup: () => rm(root, { recursive: true, force: true }),
  };
}
