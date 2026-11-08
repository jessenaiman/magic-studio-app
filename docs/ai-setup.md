# AI setup

Magic ships no API keys and stores no provider credentials. The app's AI is three
separate layers, each with its own owner:

| Layer | What it does | Where it is configured | Who holds the account |
|---|---|---|---|
| Chat agent | the in-app agent that reads the project and runs commands | `~/.config/magic/agent.json` (`agent`, `provider`, `model`, `thinking`, `extra_args`) | the agent CLI you install (`codex`, `opencode`, `claude`, …) |
| Images (Scribbles) | character, prop and storyboard images | `config.toml` → `[image] provider`, `[scribbles] provider`, `model` | the Codex subscription, through `codex login` |
| Craft AI roles | `fidget`, `scribbles`, `scribbles-mark` (newer tool layer) | shared store `~/.config/ArtCraft/ai/connections.json` | whichever connection the store binds |

There is no settings screen for the first two on purpose: credentials never enter
Magic, so the login belongs to the tool that owns it. The third store is written by
the ArtCraft launcher (or a Craft app that exposes a connection panel); Magic only
reads it.

## Quick start

1. Install and sign in once. Image generation runs through Codex, so this step is
   required even when you chat with another agent:

   ```sh
   codex login            # "Logged in using ChatGPT"
   codex login status     # confirm
   ```

2. Select the chat agent and model, and save it:

   ```sh
   magic-cli exec agent.models '{"agent":"codex"}'   # list ids for an agent
   magic-cli exec agent.use \
     '{"agent":"codex","provider":"codex","model":"gpt-6-luna"}'
   ```

   `agent.use` edits the current window's selection and writes
   `~/.config/magic/agent.json` in one step. Use `agent.config` to preview a
   selection without saving, and `agent.persist` to save the current selection.

3. Keep image generation on the Codex subscription. These are the defaults; the
   keys are only needed when you change them:

   ```sh
   magic-cli exec prefs.set '{"key":"scribbles.model","value":"gpt-6-luna"}'
   magic-cli exec prefs.get '{"key":"image.provider"}'   # "codex"
   ```

4. Verify:

   ```sh
   magic-cli exec agent.config '{}'
   ```

   The reply lists `agent`, `provider`, `model`, `default`, the installed agent
   catalog, and the `craft_ai` role status.

## Choosing a different agent

Any installed agent from the catalog works. `magic-cli exec agent.config '{}'`
lists them with `installed` and `headless` flags; agents without a headless mode
cannot answer from the window. Point `agent.use` at the name:

```sh
magic-cli exec agent.models '{"agent":"opencode"}'      # ids for that agent
magic-cli exec agent.use '{"agent":"opencode","provider":"opencode","model":"…"}'
```

An empty or omitted key keeps the value already selected.

## Craft AI roles

`agent.config` reports three roles: `fidget` (text), `scribbles` (image) and
`scribbles-mark` (detection). They read the shared store at
`~/.config/ArtCraft/ai/connections.json` (override the folder with
`CRAFT_AI_DIR`). The file is versioned JSON:

```json
{
  "schema_version": 1,
  "connections": [
    {
      "id": "conn-openai",
      "label": "OpenAI-compatible",
      "provider": "openai",
      "endpoint": "https://api.openai.com/v1",
      "locality": "remote",
      "auth": { "kind": "api_key", "secret": "OPENAI_KEY" },
      "billing": "metered",
      "models": []
    }
  ],
  "defaults": [
    { "role": "fidget", "connection": "conn-openai", "model": "…" },
    { "role": "scribbles", "connection": "conn-openai", "model": "…" }
  ]
}
```

Secrets are references, never values: `CRAFT_AI_SECRET_OPENAI_KEY` (the
`CRAFT_AI_SECRET_` prefix plus the reference, upper-cased) or the OS keychain.
Models must be listed on the connection with the capabilities their roles need.

Two cautions before you rely on this:

- The store path is still marked **"a proposal until it is settled with the
  launcher's maintainers"** in `craft-ai-integrations`
  (`crates/ai-native/src/paths.rs`). Treat the file as an interface that may move.
- `unavailable — no model is chosen` for those roles is expected until the store
  exists. It does **not** block image generation: Scribbles calls
  `image_generation::generate` (the Codex path) directly.

## Version note

The CLI you run must be built from the same checkout as the desktop app. Settings
keys are added over time (`scribbles.model` was one), and an older `magic-cli`
rejects them with `unknown setting`. After pulling:

```sh
cargo install --locked --path apps/magic-cli --force
magic-cli exec prefs.schema        # the keys this binary knows
```

## Troubleshooting

| Symptom | Cause | Fix |
|---|---|---|
| `codex: command not found` | Codex CLI not installed | install it, then `codex login` |
| `codex login status` is not logged in | no subscription session | `codex login` |
| `unknown setting 'scribbles.model'` | older `magic-cli` | rebuild/reinstall the CLI from this checkout |
| `craft_ai … unavailable — no model is chosen` | no shared store | configure the launcher's AI connection, or write `connections.json` |
| chat says "saved" but reverts on restart | a client calling `agent.config` without saving | call `agent.use` (or `agent.config` then `agent.persist`) |
