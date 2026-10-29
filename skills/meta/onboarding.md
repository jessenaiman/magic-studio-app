# Magic Studio onboarding — meta skill

## When to use

Use for a first greeting or an open-ended request such as “what can I make?”
If the user already gave an actionable request, begin that work instead of
repeating onboarding or installing anything.

## Orient to the installed app

1. Start with `magic` for the desktop application, not a source bootstrap.
2. Discover actual operations with `magic-cli --help` and `magic-cli commands`.
3. If a project is involved, select it explicitly with `magic-cli studio open
   /absolute/Song.magic`; inspect its state and current view before editing.
4. Explain the four creative areas in a few sentences:
   **Storyboard** plans scenes and artwork, **Animation** prepares movement,
   **Lyrics** designs timed words, and **Studio** composes the HyperFrames video.
5. Explain Sproket/Gaffer only when conversation or artwork is relevant.
   Backstage is the separate observer, not another production step.

For the exact CLI/MCP contract, read
[the Magic skill](../../agent-harness/cli_anything/magic/skills/SKILL.md).
If Magic is absent or the user explicitly wants a development installation, use
[Magic Studio setup](../../agent-harness/cli_anything/magic/README.md).
Do not reinstall Python, download models, or configure paid providers merely to
introduce the app.

## Begin with what the user has

Ask one useful question if the intended result is unclear: what song, recording,
footage, storyboard, or finished format are they starting with?
Preserve supplied lyrics, timestamps, artwork, and original media. Existing
transcription should be imported rather than silently regenerated. A prepared
project is not creative approval or a finished video.

Offer at most three short starters appropriate to the actual project:

- “Use my song and timed lyrics to prepare a storyboard draft.”
- “Help me prepare this character for animation.”
- “Design the lettering for this song section.”

Describe the next visible result and any required choice. Do not promise an
unverified provider, free output, completion time, or automatic full production.

## When a reusable production pipeline is requested

Python tools, YAML manifests, and stage skills are part of Magic Studio's
production toolkit. Use them when the task calls for that workflow, not as a
mandatory app-launch check. Read [the agent guide](../../AGENT_GUIDE.md), choose
the relevant `pipeline_defs/` manifest, then run its capability discovery in the
prepared source environment:

```sh
python -c "from tools.tool_registry import registry; import json; registry.discover(); print(json.dumps(registry.provider_menu_summary(), indent=2))"
```

Lead with observed working capabilities. Read missing-tool setup instructions
from the registry and offer only relevant unlocks; installations, credentials,
model downloads, and paid work need the user's applicable authorization.
Do not infer availability from the presence of source files.

For pipeline composition, distinguish Remotion and HyperFrames when both are
available; select the runtime at proposal, with brief-specific tradeoffs and
approval. The desktop Studio remains HyperFrames. See
[the runtime decision matrix](../core/hyperframes.md).
