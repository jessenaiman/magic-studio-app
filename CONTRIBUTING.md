# Contributing to OpenMontage

Thank you for improving OpenMontage. We do not require a Contributor License Agreement (CLA), Developer Certificate of Origin sign-off, or any separate contribution contract.

By submitting a pull request, you confirm that you have the right to contribute its contents. Contributions must be compatible with the repository's [AGPLv3 license](LICENSE), and accepted contributions remain available under that license.

Keep pull requests focused. Moppet agents work in a named linked worktree and branch matching their Moppet name (for example, `grober`). Keep the main checkout clean; do not commit implementation directly to `main`.

Issues use four sections: What and Where, Why, How, and Success criteria.
Use brief bullets. Review API-created issues against the same format.

New first-party skills must use `magic-` names and matching folder names
(for example, `magic-to-hyperframes`). Preserve official third-party skills,
including HyperFrames, verbatim; do not rename or paraphrase them.
Target 100 lines per first-party instruction document; maximum 200 lines.
MD013 checks line width (80 characters by default), not document line count;
the file-line limit still needs a suitable native tool, not a custom script.
Audit only our `magic-*` skill folders with
`npx --yes markdownlint-cli2@0.24.0`; do not use blanket scans or autofix.
Install the official hook with `pre-commit install`; it checks matching staged
Markdown only. Run `pre-commit run markdownlint-cli2 --all-files` for an audit.
Use Node.js 22+ for markdownlint-cli2; Ruff remains Python-only.

Submit a pull request with the problem, resulting behavior, commands actually executed, observed results and remaining limitations. Obtain review before merging; agents must not merge their own unreviewed changes.

Add behavioral tests only after Jesse personally navigates or observes the real UI and explicitly approves each expected workflow. Use the native WebdriverIO/installed `magic-cli` runner for approved workflows. Before approval, record proposed command/expectation chains and observations without presenting them as approved regression coverage. Retain ordinary compiler, syntax and lint checks. Preserve intentional deletions and never restore retired architecture merely to satisfy old tests.
