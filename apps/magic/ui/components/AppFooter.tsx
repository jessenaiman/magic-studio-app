import { useEffect, useState } from "react";
import type { ViewMode } from "../App";
import type { StoryboardData, WorkshopData } from "../api";
import { workspaceDrafts } from "../workspace-drafts";

/**
 * The React/Tauri shell's only application footer.
 *
 * It is rendered by `App` as a sibling of `.app-view-host`, so it is created once and
 * survives every view switch. Embedded legacy workspaces (`/workspaces/*.html`) never
 * render their own footer: `apps/magic-cli/ui/style.css` keeps the legacy `#app-footer`
 * hidden until `app.js` marks a top-level document with `.magic-browser`, and `app.js`
 * skips its footer renderer when `window.self !== window.top`; the copies of index.html
 * used for the project and animation workspaces inherit both guards.
 *
 * All three items read one shell-side source each, and none of them is log text:
 * - stages: `data.status.prerequisites` from the same `project.snapshot` payload
 *   (`session_snapshot` in Tauri), built from the song/transcript/storyboard/animate/studio
 *   reports by `crates/engine/src/cmd/project/workshop.rs`; the storyboard item also reads
 *   the payload's `storyboard` report so #122's `untraced` assets count
 * - context: the active view plus that same payload
 * - save state: `workspaceDrafts()`, the shell's single draft aggregator also used by
 *   `desktop.ts` (ui.drafts / lifecycle) and `FilmExportButton`; it is not a copy of
 *   the legacy app.js footer state.
 *
 * Engine/command errors never become footer text: `App` renders `showToast` output as a
 * transient toast, while this footer stays status-only.
 *
 * The instance/build item the legacy footer shows ("Studio <pid> · <branch>") is omitted:
 * that identity came from the retired UI server's `startup` reply, and the Tauri shell has no
 * equivalent (`project.identity` returns a canonical project path only). The owner asked for a
 * real label or nothing, so nothing is shown rather than a permanent "Connecting…".
 *
 * Browser mode (`magic-cli ui`, a top-level `apps/magic-cli/ui/index.html`) has no React shell,
 * so the legacy `AppFooterController` in `apps/magic-cli/ui/app.js` stays the one footer there.
 * The split is by shell: exactly one of the two footers is ever shown.
 */

type Tone = "ready" | "needs-attention" | "failed" | "working" | "idle";

interface StageDefinition {
  id: string;
  label: string;
  view: ViewMode;
}

const STAGES: ReadonlyArray<StageDefinition> = [
  { id: "song", label: "Song", view: "project" },
  { id: "lyrics", label: "Lyrics", view: "design" },
  { id: "transcript", label: "Transcript", view: "project" },
  { id: "storyboard", label: "Storyboard", view: "storyboard" },
  { id: "animate", label: "Animate", view: "animation" },
  { id: "studio", label: "Studio", view: "studio" },
];

// `needs-work` is the engine's incomplete state. `untraced` is the state issue #122 adds
// for storyboard artwork with no project asset; until that report ships the fallback below
// reads only fields the engine already returns (`complete`, `status`, `count`), and no new
// engine command is invented here.
const INCOMPLETE_STATES = new Set(["needs-work", "needs_work", "untraced", "incomplete", "review"]);

interface Prerequisite {
  id?: string;
  state?: string;
  detail?: string;
}

interface Props {
  view: ViewMode;
  data: WorkshopData | null;
  projectPath: string | null;
  onNavigate: (view: ViewMode) => void;
  onToast: (message: string) => void;
}

/** `workshop_status` emits prerequisites as an object; older payloads used an array. */
function prerequisites(data: WorkshopData | null): Record<string, Prerequisite> {
  const raw = data?.status?.prerequisites;
  if (!raw) return {};
  if (Array.isArray(raw)) {
    return Object.fromEntries(raw.filter((entry): entry is Prerequisite & { id: string } => !!entry?.id).map(entry => [entry.id, entry]));
  }
  return raw;
}

function countUntraced(value: StoryboardData["untraced"] | undefined): number {
  if (Array.isArray(value)) return value.length;
  return typeof value === "number" && Number.isFinite(value) ? value : 0;
}

/**
 * Issue #122 reports untraced storyboard artwork at the top level and per page. Prefer the
 * top-level count when the report has one, otherwise sum the pages; this only displays what
 * `storyboard.check` returns.
 */
function untracedCount(storyboard: StoryboardData | undefined): number {
  const top = countUntraced(storyboard?.untraced);
  if (top) return top;
  return (storyboard?.pages ?? []).reduce((total, page) => total + countUntraced(page.untraced), 0);
}

function toneFor(state: string | undefined): Tone {
  if (state === "ready") return "ready";
  if (state === "failed") return "failed";
  if (state && INCOMPLETE_STATES.has(state)) return "needs-attention";
  return "idle";
}

interface StageStatus {
  tone: Tone;
  state: string;
  detail: string;
}

function stageStatus(stage: StageDefinition, data: WorkshopData | null): StageStatus {
  const prerequisite = prerequisites(data)[stage.id];
  const engineStage = data?.status?.stages?.find(entry => entry.id === stage.id);
  let state = prerequisite?.state ?? engineStage?.state ?? (engineStage?.done ? "ready" : "not-started");
  let detail = prerequisite?.detail ?? engineStage?.why ?? "";

  // The storyboard report already drives the engine's prerequisite state. If a report says
  // the board is incomplete while the prerequisite still says ready, trust the report; and
  // surface #122's untraced assets whenever the report carries them.
  if (stage.id === "storyboard") {
    const untraced = untracedCount(data?.storyboard);
    if (untraced) {
      state = "untraced";
      detail = `${untraced} untraced asset${untraced === 1 ? "" : "s"}${detail ? ` · ${detail}` : ""}`;
    } else if (data?.storyboard?.complete === false && state === "ready") {
      state = "needs-work";
    }
  }

  return { tone: toneFor(state), state, detail };
}

function plural(count: number, word: string): string {
  return `${count} ${word}${count === 1 ? "" : "s"}`;
}

interface ProjectFacts {
  pages: number;
  objects: number;
  untraced: number;
}

function projectFacts(data: WorkshopData | null): ProjectFacts {
  const storyboard = data?.storyboard;
  return {
    pages: storyboard?.page_count ?? storyboard?.pages?.length ?? 0,
    objects: data?.assets?.assets?.length ?? 0,
    untraced: untracedCount(storyboard),
  };
}

/** Mirrors the legacy footer's storyboard context: pages, object count, plus #122's untraced. */
function storyboardLine(facts: ProjectFacts): string {
  return [
    facts.pages ? `Storyboard · ${plural(facts.pages, "page")}` : "Storyboard · not built yet",
    facts.untraced ? plural(facts.untraced, "untraced asset") : null,
    plural(facts.objects, "object"),
  ].filter(Boolean).join(" · ");
}

/** The active view's context, read from the same project snapshot as the stages. */
function contextText(view: ViewMode, data: WorkshopData | null, projectPath: string | null): string {
  if (view === "mayhem") return "Mayhem · code, usage and tool reports";
  if (!projectPath || view === "home") {
    return view === "gallery" ? "Gallery · browse artwork, or open a project" : "Choose a project or start a new song";
  }

  const facts = projectFacts(data);
  switch (view) {
    case "gallery":
      return `Gallery · ${plural(facts.objects, "object")} in this project`;
    case "studio": {
      const timeline = data?.status?.film_timeline;
      return timeline?.exists ? timeline.path || "Final FilmCraft timeline linked" : "No final FilmCraft timeline linked";
    }
    case "animation": {
      const animate = data?.status?.stages?.find(entry => entry.id === "animate");
      const subjects = animate?.assets?.length ?? 0;
      return subjects
        ? `Animation · ${plural(subjects, "animated subject")} · ${plural(facts.objects, "object")}`
        : "Animation · no animated subject chosen";
    }
    default:
      return storyboardLine(facts);
  }
}

/** Reads the shell's one draft aggregator; the interval keeps the footer live as editors change. */
function useDraftState() {
  const [drafts, setDrafts] = useState(() => workspaceDrafts());
  useEffect(() => {
    const refresh = () => setDrafts(workspaceDrafts());
    refresh();
    const timer = window.setInterval(refresh, 1000);
    const onFocus = () => refresh();
    window.addEventListener("focus", onFocus);
    document.addEventListener("visibilitychange", onFocus);
    return () => {
      window.clearInterval(timer);
      window.removeEventListener("focus", onFocus);
      document.removeEventListener("visibilitychange", onFocus);
    };
  }, []);
  return drafts;
}

export const AppFooter: React.FC<Props> = ({ view, data, projectPath, onNavigate, onToast }) => {
  const drafts = useDraftState();

  const go = (stage: StageDefinition) => {
    if (!projectPath) {
      onToast("Open a project before opening a workflow stage");
      return;
    }
    onNavigate(stage.view);
  };

  const save = drafts.saving
    ? { tone: "working" as Tone, label: "Saving…", title: "A document is saving" }
    : drafts.dirty
      ? { tone: "needs-attention" as Tone, label: "Unsaved edits", title: drafts.drafts.join(" · ") || "A document has unsaved edits" }
      : projectPath
        ? { tone: "ready" as Tone, label: "Project saved", title: "No tracked document edits are waiting to be saved" }
        : { tone: "idle" as Tone, label: "Not saved", title: "No project is open" };

  const context = contextText(view, data, projectPath);
  // One stage can be current; project/gallery/mayhem have no single workflow stage.
  const currentStage = view === "storyboard" ? "storyboard"
    : view === "animation" ? "animate"
      : view === "studio" ? "studio"
        : view === "design" || view === "motion" ? "lyrics"
          : null;

  return (
    <footer id="app-footer" className="app-footer" role="contentinfo" aria-label="Application status footer">
      <nav id="footer-stages" className="app-footer-stages" aria-label="Workflow stages">
        {STAGES.map(stage => {
          const status = stageStatus(stage, data);
          return (
            <button
              key={stage.id}
              id={`footer-stage-${stage.id}`}
              type="button"
              className="app-footer-stage"
              data-current={currentStage === stage.id ? "true" : undefined}
              aria-label={`${stage.label}: ${status.state.replace(/-/g, " ")}`}
              title={`${stage.label} · ${status.detail || status.state.replace(/-/g, " ")}`}
              onClick={() => go(stage)}
            >
              <span className={`app-footer-dot ${status.tone}`} aria-hidden="true" />
              <span className="app-footer-stage-label">{stage.label}</span>
            </button>
          );
        })}
      </nav>
      <div id="footer-context" className="app-footer-context" aria-live="polite" title={context}>{context}</div>
      <span id="footer-save-state" className={`app-footer-save app-footer-save-${save.tone}`} title={save.title}>
        <span className={`app-footer-dot ${save.tone}`} aria-hidden="true" />
        {save.label}
      </span>
    </footer>
  );
};
