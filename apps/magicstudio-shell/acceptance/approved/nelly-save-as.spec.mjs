// Approved case: Jesse, 2026-10-28T15:07:10-04:00, recorded in issue #10.
// Actions are installed magic-cli commands routed to the launched window;
// WDIO only reads that window's page. The native GTK chooser is out of scope.
import assert from 'node:assert/strict';
import { join } from 'node:path';
import { createWorkflow } from '../workflow.mjs';

const approval = {
  reference: 'https://github.com/jessenaiman/magic-studio-app/issues/10#issuecomment-6478139342',
  approvedBy: 'Jesse Naiman',
  approvedAt: '2026-10-28T15:07:10-04:00',
};

// Wait for the page header to name the project, then report what the page shows.
// WebKitWebDriver's rendered-text call returns "" for this header, so read its text
// content; the step screenshot is the visual check.
const headerText = (browser) => browser.$('#project-name').getProperty('textContent');
async function visible(browser, name) {
  await browser.waitUntil(async () => (await headerText(browser)) === name,
    { timeout: 60000, timeoutMsg: `Header never showed "${name}"` });
  return {
    header: await headerText(browser),
    title: await browser.getTitle(),
    footer: await browser.$('#footer-instance').getAttribute('title'),
  };
}

// A result without an endpoint ran headless, not in the window under test.
const reachedWindow = (actual) => {
  assert.equal(actual.exitCode, 0, actual.stderr);
  assert.match(actual.result?.endpoint ?? '', /^unix:/, 'Command did not reach a live window');
  return actual.result.endpoint;
};

describe('Nelly: open, Save As via magic-cli, reopen', () => {
  it('follows the approved chain', async () => {
    const flow = await createWorkflow({ browser, approval });
    const copy = join(flow.root, 'Nelly Acceptance Copy.magic');
    const source = await flow.fileEvidence(flow.project);

    await flow.ui({
      name: 'Nelly opens in the launched window',
      action: { description: 'App launched with the Nelly copy as its argument', check: (b) => visible(b, 'Nelly') },
      expectIdentity: { description: 'Title is "Nelly · Magic Studio"', check: ({ observed }) => assert.equal(observed.title, 'Nelly · Magic Studio') },
      expectUI: { description: 'Footer names a build', check: ({ observed }) => assert.match(observed.footer, /PID \d+/) },
    });

    await flow.cli({
      name: 'ui feedback reaches the launched window',
      args: ['ui', 'feedback'],
      expectExit: { description: 'Exit 0 via a window endpoint', check: reachedWindow },
      expectResult: { description: 'Connected, ready, owns the writer lease', check: ({ result }) => {
        assert.equal(result.connection, 'connected');
        assert.equal(result.ui_ready, true);
        assert.equal(result.writer_owner, true);
      } },
      expectIdentity: { description: 'Active project is the copy; binary is this checkout', check: ({ result }) => {
        assert.equal(result.active_project, flow.project);
        assert.equal(result.identity.executable, process.env.MAGICSTUDIO_SHELL_BIN);
      } },
      expectUI: { description: 'Header still shows Nelly', check: (_, { browser: b }) => visible(b, 'Nelly') },
    });

    await flow.cli({
      name: 'Save As to a new path',
      args: ['project', 'save', `path=${copy}`],
      modifiesFiles: true,
      expectExit: { description: 'Exit 0 via a window endpoint', check: reachedWindow },
      expectResult: { description: 'Result names the new path', check: ({ result }) => assert.equal(result.project, copy) },
      expectIdentity: { description: 'Title becomes "Nelly Acceptance Copy · Magic Studio"', check: async (_, { browser: b }) => {
        const seen = await visible(b, 'Nelly Acceptance Copy');
        assert.equal(seen.title, 'Nelly Acceptance Copy · Magic Studio');
        return seen;
      } },
      expectUI: { description: 'Header shows the new name', check: (_, { browser: b }) => visible(b, 'Nelly Acceptance Copy') },
      expectFiles: { description: 'New file carries the new name; Nelly copy bytes unchanged', check: async (_, { fileEvidence }) => {
        const saved = await fileEvidence(copy);
        assert.equal(JSON.parse(saved.content).name, 'Nelly Acceptance Copy');
        const after = await fileEvidence(flow.project);
        assert.equal(after.sha256, source.sha256, 'Source project bytes changed');
        return { saved: saved.sha256, source: after.sha256 };
      } },
    });

    await flow.cli({
      name: 'Reopen the saved copy',
      args: ['project', 'open', `path=${copy}`],
      targetProject: copy,
      expectExit: { description: 'Exit 0 via a window endpoint', check: reachedWindow },
      expectResult: { description: 'Opened project is the copy', check: ({ result }) => assert.equal(result.project, copy) },
      expectIdentity: { description: 'Same window keeps the copy', check: async (_, { browser: b }) => (await visible(b, 'Nelly Acceptance Copy')).title },
      expectUI: { description: 'Header shows the copy', check: (_, { browser: b }) => visible(b, 'Nelly Acceptance Copy') },
    });
  });
});
