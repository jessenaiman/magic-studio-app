# Magic Studio â€" Skill Index

> For the full agent onboarding guide, see [`AGENT_GUIDE.md`](../AGENT_GUIDE.md) in the project root.

This file lists all available Layer 2 skills and documents the 3-layer knowledge architecture.

## Knowledge Architecture

```
Layer 1: pipeline/ + pipeline_defs/      "What runs and how a production flows"
         Engine-invoked production        Data-only presets: stages, artifacts,
         scripts and preset stages        tools, review focus, gates

         ↑ referenced by ↑

Layer 2: skills/ + agents/                "How Magic Studio uses these"
         Project-specific conventions:     Artifact mappings, enhancement chain order,
         {core,creative,meta}/ + agents/    quality checklists, department instructions

         â†" references underlying tech in â†"

Layer 3: .agents/skills/                  "How the technology itself works"
         Generic API knowledge from        Correct import paths, code patterns,
         skills.sh (47 installed skills)   constraints, parameters â€" tech-agnostic
```

**How the agent uses this:**
1. The Video Production agent reads the selected preset's stage entries to see what's required
2. Layer 2 skills (this directory) teach the agent Magic Studio-specific conventions
3. Layer 3 skills (`.agents/skills/`) provide generic API knowledge, loaded on-demand

## Core Skills

| Skill | File | Trigger | Agent Skills (Layer 3) |
|-------|------|---------|----------------------|
| FFmpeg | `core/ffmpeg.md` | Video encoding, filtering, composition | `ffmpeg`, `video-toolkit` |
| Remotion | `core/remotion.md` | React-based composition, Phase 3+ | `remotion-best-practices`, `remotion` |
| HyperFrames | `core/hyperframes.md` | HTML/CSS/GSAP composition runtime — kinetic typography, music-to-video, product promos, website capture. Vendored at v0.7.17 (2026-06-27). | `hyperframes` (router) → `hyperframes-core` (contract), `hyperframes-creative` (palette/type/narration), `hyperframes-media` (TTS/BGM/SFX/captions), `hyperframes-animation` (all motion), `hyperframes-cli`, `hyperframes-registry`, `media-use`, `motion-graphics`, `music-to-video` (beats-driven), `website-to-video`, `remotion-to-hyperframes` (migration), `gsap-core`, `gsap-timeline` |
| WhisperX | `core/whisperx.md` | Transcription with word-level timestamps — default STT (offline, free) | `speech-to-text` |
| Azure STT | (tool: `azure_stt`) | Optional cloud speech-to-text, word-level timestamps — preferred when `AZURE_SPEECH_KEY` is set | `azure-speech-to-text` |
| Azure TTS | (tool: `azure_tts`) | Optional cloud neural narration (SSML prosody, express-as styles) — same Speech key as `azure_stt` | `azure-text-to-speech` |
| Subtitle Sync | `core/subtitle-sync.md` | Subtitle timing and alignment | `remotion-best-practices` |
| Color Grading | `core/color-grading.md` | FFmpeg color profiles, LUT workflow, accessibility | `ffmpeg` |

## Creative Skills

| Skill | File | Trigger | Agent Skills (Layer 3) |
|-------|------|---------|----------------------|
| Video Editing | `creative/video-editing.md` | Cut decisions, pacing, rhythm | `ffmpeg`, `video-toolkit` |
| Enhancement Strategy | `creative/enhancement-strategy.md` | Overlay placement and density | `ffmpeg` |
| Data Visualization | `creative/data-visualization.md` | Chart type selection, animation, label placement | `d3-viz`, `remotion-best-practices` |
| Video Stitching | `creative/video-stitching.md` | Multi-clip assembly, AI clip chaining, spatial composition | `ffmpeg`, `video-toolkit` |
| Video Gen Prompting | `creative/video-gen-prompting.md` | Universal video generation prompt vocabulary; **canonical 5-aspect spec** (Subject / Motion / Scene / Spatial / Camera); ~200 cinematography primitives | `ai-video-gen`, `ltx2`, `create-video` |
| â†³ Seedance Prompting | `creative/prompting/seedance-prompting.md` | **Preferred premium default.** Seedance 2.0 8-component structure, multi-shot, lip-sync, reference-to-video | `seedance-2-0`, `ai-video-gen` |
| â†³ Grok Prompting | `creative/prompting/grok-prompting.md` | Grok image/video prompting, edit flows, reference-image video | `grok-media` |
| â†³ Sora Prompting | `creative/prompting/sora-prompting.md` | Sora 2 structured template, advanced fields | `ai-video-gen` |
| â†³ VEO Prompting | `creative/prompting/veo-prompting.md` | VEO 3.1 14-component structure, art movements | `ai-video-gen` |
| â†³ LTX Prompting | `creative/prompting/ltx-prompting.md` | LTX-2 6-element structure, audio prompting | `ltx2` |
| â†³ HunyuanVideo Prompting | `creative/prompting/hunyuan-prompting.md` | HunyuanVideo formula, I2V best practices | â€" |
| Storytelling | `creative/storytelling.md` | Narrative structure, hooks, pacing, Mayer's principles | â€" |
| Sound Design | `creative/sound-design.md` | Audio ducking, LUFS targets, SFX timing, AI TTS mixing | `elevenlabs` |
| Typography | `creative/typography.md` | Font selection, text sizing, safe zones, caption styling | â€" |
| ManimCE Usage | `creative/manim-usage.md` | Scene composition, animation timing, color usage | `manimce-best-practices` |
| Image Gen Usage | `creative/image-gen-usage.md` | Prompt consistency, hero reference, batch strategy | `flux-best-practices`, `bfl-api` |
| Image Provider Usage | `creative/image-provider-usage.md` | Provider selection (FLUX/Grok/OpenAI/Recraft/stock), cost-quality tradeoffs | `flux-best-practices`, `bfl-api`, `grok-media` |
| 3D World Generation | `creative/3d-world-generation.md` | Semantic world planning, asset sourcing/generation, Blender assembly, and fidelity review | `3d-asset-generation`, `threejs-world-generation` |
| B-Roll Planning | `creative/broll-planning.md` | Stock vs. generated decision, query construction, footage evaluation | — |
| Stock Sourcing Usage | `creative/stock-sourcing-usage.md` | Pexels/Pixabay usage, parameters, licensing, integration | — |
| Scene Detect Usage | `creative/scene-detect-usage.md` | Threshold tuning, algorithm selection, content presets | â€" |
| Diagram Gen Usage | `creative/diagram-gen-usage.md` | Complexity limits, progressive building, themes | `beautiful-mermaid` |
| Music Gen Usage | `creative/music-gen-usage.md` | BPM selection, prompt engineering, duration matching | `music`, `elevenlabs` |
| Background Removal | `creative/bg-remove-usage.md` | Model selection, alpha matting, compositing workflows | â€" |
| Upscaling | `creative/upscale-usage.md` | Scale factor, model selection, face-aware upscaling | â€" |
| Face Restoration | `creative/face-restore-usage.md` | CodeFormer/GFPGAN selection, fidelity tuning, vs face_enhance | â€" |
| Lip Sync | `creative/lip-sync-usage.md` | Wav2Lip model selection, dubbing workflows, input requirements | `faceswap` |
| Talking Head Gen | `creative/talking-head-gen-usage.md` | SadTalker/MuseTalk, photo-to-video, expression tuning | `avatar-video` |
| Video Understanding | `creative/video-understand-usage.md` | Visual QA, quality gating, scene classification | `video-understand` |

## Pipeline Type Skills

Pipeline type skills provide production guidance for specific video formats, independent of the animated-explainer or talking-head pipeline.

| Skill | File | When to Use |
|-------|------|-------------|
| Short-Form | `creative/short-form.md` | TikTok, Reels, Shorts â€" vertical 9:16, under 60s |
| Long-Form | `creative/long-form.md` | YouTube 10+ min â€" chapters, retention, end screens |
| Screen Recording | `creative/screen-recording.md` | Code walkthroughs, tutorials, software demos |
| Animation Pipeline | `creative/animation-pipeline.md` | Motion graphics, easing, transitions, composition |
| 3D World Generation | `creative/3d-world-generation.md` | Continuous Three.js terrain worlds with semantic regions, explicit blockout/production tiers, licensed GLTF/PBR assets, diagnostics, and deterministic camera paths |
| Character Animation | `pipeline_defs/character-animation.yaml` | Rigged local cartoon characters, pose libraries, action timelines, SVG/Canvas/Remotion/HyperFrames rendering |
| Cinematic | `creative/cinematic.md` | Letterbox, film pacing, layered audio, color grading |

## Pipeline Presets

`pipeline_defs/*.yaml` holds data-only production presets: stage order, tools,
produced artifacts, review focus, success criteria, and approval gates. The
Video Production department agent (`agents/video-production.md`) reads the
selected preset and executes its stages serially. There are no per-pipeline
instruction files; the preset is the stage contract.

| Preset | Category | Description |
|--------|----------|-------------|
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

## Meta Skills

Cross-cutting skills that apply to all pipelines:

| Skill | File | Purpose |
|-------|------|---------|
| Onboarding | `meta/onboarding.md` | First-interaction greeting, capability discovery, starter prompts |
| Reviewer | `meta/reviewer.md` | Self-review protocol after every stage |
| Skill Creator | `meta/skill-creator.md` | Dynamically create new skills during pipeline runs |
| Animation Runtime Selector | `meta/animation-runtime-selector.md` | Choose render runtime + animation library per scene |
| Creative Intake | `meta/creative-intake.md` | Shape an open request into a producible brief |
| Video Reference Analyst | `meta/video-reference-analyst.md` | Inspect a supplied reference video before proposing |
| Taste Direction | `meta/taste-direction.md` | Convert a brief into taste dials, anti-patterns, and reference strategy for proposal/atelier work |
| Bespoke Composition (Atelier) | `meta/bespoke-composition.md` | Hand-author a composition from scratch (hero work) — no stock scene-types; routes art-direction → motion principles → engine mechanics → atelier render |

## Installed Agent Skills (Layer 3)

All agent skills live in `.agents/skills/` and are managed via `npx skills add`.
Claude Code accesses them via symlinks in `.claude/skills/`.

| Category | Installed Skills | Source |
|----------|-----------------|--------|
| **Video Composition** | `remotion-best-practices`, `remotion`, `hyperframes` (router), `hyperframes-core`, `hyperframes-creative`, `hyperframes-media`, `hyperframes-animation`, `hyperframes-cli`, `hyperframes-registry`, `media-use`, `motion-graphics`, `music-to-video`, `remotion-to-hyperframes`, `website-to-video` | `remotion-dev/skills`, `digitalsamba/claude-code-video-toolkit`, `heygen-com/hyperframes` (vendored v0.7.17, see `.agents/skills/hyperframes/PROVENANCE.md`) |
| **Video Processing** | `ffmpeg`, `video-toolkit` | `digitalsamba/claude-code-video-toolkit` |
| **TTS & Audio** | `text-to-speech`, `speech-to-text` (whisper, default STT), `azure-speech-to-text` (optional cloud STT), `music`, `sound-effects`, `elevenlabs`, `fish-audio-tts`, `agents`, `setup-api-key` | `elevenlabs/skills`, `digitalsamba/claude-code-video-toolkit`, local Magic Studio skill |
| **Image Generation** | `flux-best-practices`, `bfl-api`, `grok-media` | `black-forest-labs/skills`, local Magic Studio skill |
| **Math Animation** | `manimce-best-practices`, `manimgl-best-practices`, `manim-composer` | `adithya-s-k/manim_skill` |
| **3D Graphics** | `threejs-world-generation` (Magic Studio semantic-world workflow), `threejs-animation`, `threejs-fundamentals`, `threejs-geometry`, `threejs-interaction`, `threejs-lighting`, `threejs-loaders`, `threejs-materials`, `threejs-postprocessing`, `threejs-shaders`, `threejs-textures` | Local Magic Studio skill + `cloudai-x/threejs-skills` |
| **Diagrams** | `beautiful-mermaid`, `d3-viz` | `intellectronica/agent-skills`, `davila7/claude-code-templates` |
| **Animation** | `framer-motion`, `lottie-bodymovin` | `pproenca/dot-skills`, `dylantarre/animation-principles` |
| **Design** | `tailwind-design-system`, `web-design-guidelines`, `vercel-react-best-practices`, `vercel-composition-patterns` | `wshobson/agents`, `vercel-labs/agent-skills` |
| **AI Video (HeyGen)** | `heygen`, `avatar-video`, `create-video`, `faceswap`, `ai-video-gen`, `video-download`, `video-edit`, `video-translate`, `video-understand`, `visual-style` | `heygen-com/skills` |
| **AI Video/Image/TTS/Avatar (Kling Official)** | `kling-official` - official direct API auth, Classic/Turbo/Omni task protocols, multi-reference Omni syntax, internal Elements/Account Usage helpers, callback notes, TTS voice parameters, avatar/lip-sync face selection, error handling, and cost governance for `kling_official_video` / `kling_official_image` / `kling_tts` / `kling_avatar` / `kling_lip_sync` | Local Magic Studio skill |
| **AI Video (Premium)** | `seedance-2-0` — preferred premium default (cinematic, trailer, multi-shot, lip-sync, synced audio); accessed via `seedance_video` (fal.ai) or `heygen_video` Avatar Shots; `seedance-2-5` — 4–30 s clips, 50 multimodal references, prompt contract (section order, `Hard cut` breakdown, continuity locks, asset method) | Local Magic Studio skill |
| **Infrastructure** | `acestep`, `ltx2`, `playwright-recording` | `digitalsamba/claude-code-video-toolkit` |
