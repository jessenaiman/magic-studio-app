# Magic Studio architecture

## Desktop and observer

The desktop app is the Tauri shell in `apps/src-tauri/`, with Storyboard,
Animation, Lyrics, and Studio views. Its embedded UI, CLI, and MCP share the Rust
engine registry in `crates/engine/src/commands.rs`. Project persistence and domain
validation belong to the engine. The CLI-Anything Python harness is an adapter,
not a second native installer or project implementation.

Backstage (`apps/backstage/`) is a separate Rust/Ratatui observer for saved project
references, PR handoffs, worktrees, native audit evidence, and settings. See
[project context](../PROJECT_CONTEXT.md), [command architecture](../agent-harness/MAGIC.md),
and [Backstage](../apps/backstage/README.md) for these contracts. The
[source-installation guide](../agent-harness/cli_anything/magic/README.md) owns
app build and installation instructions.

## Reusable production pipeline

The remaining sections describe the retained production layer, not requirements
for launching the desktop app. The Video Production department agent reads the
selected pipeline preset and executes its stages with the `pipeline/` scripts.
Creative orchestration belongs to the department agents; the scripts provide
the mechanical steps.

### High-Level Flow

```
User gives topic/idea
        |
        v
Agent reads pipeline preset (YAML)
        |
        v
For each stage:
   1. Agent follows the Video Production department agent (agents/)
   2. Agent runs the pipeline/ step for the stage
   3. Agent records canonical artifacts
   4. Agent self-reviews using meta/reviewer skill
   5. Human approval gate (if configured)
        |
        v
Final video output
```

### Repository Layout

```
magic-studio-app/
├── apps/
│   ├── src-tauri/            # Tauri shell: Rust engine commands + shell UI
│   └── backstage/            # Rust/Ratatui observer
├── crates/engine/            # Shared Rust engine (commands, motion, projects)
├── agent-harness/            # CLI-Anything magic harness (magic-cli, MCP)
├── agents/                   # Department agent files (storyboard, animation,
│                             # lyrics, video-production, mayhem)
├── pipeline/                 # Engine-invoked production scripts (animation,
│   │                         # layout, rigging, import/compare steps)
│   └── templates/            # Rig sheets, caption/lyrics templates, deck template
├── pipeline_defs/            # Data-only pipeline presets (stages, artifacts,
│                             # tools, review focus, gates)
├── skills/                   # Layer 2: Magic Studio conventions
│   ├── core/                 # FFmpeg, Remotion, WhisperX, color grading skills
│   ├── creative/             # Video editing, enhancement, data viz, prompting
│   └── meta/                 # reviewer, skill-creator, runtime selection
├── .agents/skills/           # Layer 3: external technology skills
├── remotion-composer/        # Node.js/React — Remotion composition renderer
└── docs/                     # Guides and workflows
```

### Core Principle: Agent-First Orchestration

There is **no Python orchestrator**. The LLM agent:
- Reads the pipeline preset to know the stage order
- Follows the Video Production department agent for stage execution
- Runs the `pipeline/` scripts, evaluates results, makes creative decisions
- Records canonical artifacts between stages

### Pipeline Presets

Each production format is a data-only YAML preset in `pipeline_defs/`:

```yaml
name: animated-explainer
version: "2.0"
category: generated
default_checkpoint_policy: guided

orchestration:
  mode: executive-producer
  budget_default_usd: 2.00
  max_revisions_per_stage: 3

stages:
  - name: research
    produces: [research_brief]
    tools_available: []
    checkpoint_required: false
    human_approval_default: false
    review_focus: [...]
    success_criteria: [...]
  # ... through publish
```

Available presets:

| Preset | Category | Description |
|----------|----------|-------------|
| `animated-explainer` | generated | AI-produced explainer with research, narration, visuals, music |
| `animation` | animation | Motion graphics, kinetic typography |
| `avatar-spokesperson` | talking_head | Avatar-driven presenter videos |
| `character-animation` | animation | Local rigged cartoon characters (rigs, poses, actions) |
| `cinematic` | cinematic | Trailer, teaser, mood-driven edits |
| `clip-factory` | custom | Batch short-form clips from long source |
| `documentary-montage` | documentary | Footage-led documentary montages |
| `hybrid` | hybrid | Source footage + AI-generated support visuals |
| `podcast-repurpose` | hybrid | Podcast highlights to video |
| `screen-demo` | screen_recording | Software screen recordings and walkthroughs |
| `talking-head` | talking_head | Footage-led speaker videos |

Most presets follow a canonical stage flow:

```
research → proposal → script → scene_plan → assets → edit → compose → publish
```

Each stage declares `tools_available`, `produces`, `review_focus`,
`success_criteria`, and whether a human approval gate is required.
Specialized presets insert domain stages — `character-animation` adds
`character_design` and `rig_plan` before `scene_plan`.

### Composition Runtimes

Remotion uses the React composition stack (`remotion-composer/`); HyperFrames
uses HTML/CSS/GSAP; FFmpeg handles direct media assembly. The desktop Studio
targets HyperFrames. Motion-required work must keep real motion; never silently
substitute a still-led treatment when a renderer fails.
