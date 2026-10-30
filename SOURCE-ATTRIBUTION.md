# Source attribution

Magic Studio is a project by Jesse Naiman. It incorporates and modifies OpenMontage code and draws on its skills and production workflow concepts. OpenMontage was originally released by calesthio and its contributors: https://github.com/calesthio/OpenMontage. The source repository used for this migration is https://github.com/ontariodungeons/magic-studio at commit `2604813cd19447af23646842eb2f24f0a7684591`, together with its working-tree deletions. Its prior history and contributor attribution remain in that repository and the original local Git database. This repository begins separately to exclude historical creative media.

The root LICENSE preserves the upstream GNU AGPL version 3. Existing component notices and font licenses are retained in their original locations. The Cargo workspace currently declares `MIT OR Apache-2.0`; this inherited declaration is inconsistent with the repository license and is retained pending clarification, not as a statement of relicensing.

## Upstream workflow reference

The BGM Montage workflow comparison references the original
[OpenMontage integration, PR #751](https://github.com/calesthio/OpenMontage/pull/751).
It is upstream history, not a Magic Studio pull request.

## Embedded Backstage terminal

The desktop bundles xterm.js 6.0.0 and @xterm/addon-fit 0.11.0 (MIT).
Upstream: https://github.com/xtermjs/xterm.js . Their full MIT notices are
retained in `apps/magicstudio-shell/ui/vendor/backstage/` by the frontend build.
The native PTY adapter uses portable-pty 0.9.0 (MIT), part of WezTerm:
https://github.com/wez/wezterm/tree/main/pty .
