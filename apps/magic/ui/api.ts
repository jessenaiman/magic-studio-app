import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { arch, eol, exeExtension, family, locale, platform, type as osTypeFn, version } from "@tauri-apps/plugin-os";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { formatOsInfo, type OsInfo } from "./os-info";
import { notifyTaskComplete } from "./notification";
import { isCentered, isTopRight, moveWindow, Position, type Rect } from "./positioner";

export type { OsInfo };
export { isMacOSPlatform, isWindowsPlatform, isUnixFamily, modifierLabel } from "./os-info";

export async function getOsInfo(): Promise<OsInfo> {
  return formatOsInfo({
    platform: platform(),
    family: family(),
    arch: arch(),
    version: version(),
    osType: osTypeFn(),
    exeExtension: exeExtension(),
    eol: eol(),
    locale: await locale(),
  });
}

export type { Rect } from "./positioner";
export { Position } from "./positioner";

export async function getWindowPosition(): Promise<{ window: Rect; screen: Rect }> {
  const current = getCurrentWindow();
  const monitor = await currentMonitor();
  if (!monitor) throw new Error("No monitor reported for the main window");
  const [at, size] = await Promise.all([current.outerPosition(), current.outerSize()]);
  return {
    window: { x: at.x, y: at.y, width: size.width, height: size.height },
    screen: { x: monitor.position.x, y: monitor.position.y, width: monitor.size.width, height: monitor.size.height },
  };
}

// Snap the window to the top-right corner of its screen, or back to the center when it is already there.
// The plugin resolves even when the window manager ignores the request (Wayland clients cannot place
// themselves), so the result is read back and a window that did not move is an error, not a success.
export async function toggleWindowSnap(): Promise<"top-right" | "center"> {
  const before = await getWindowPosition();
  const target = isTopRight(before.window, before.screen) ? "center" : "top-right";
  await moveWindow(target === "center" ? Position.Center : Position.TopRight);
  await new Promise((resolve) => setTimeout(resolve, 300));
  const after = await getWindowPosition();
  const arrived = target === "center" ? isCentered(after.window, after.screen) : isTopRight(after.window, after.screen);
  if (!arrived) {
    throw new Error(`The window manager did not move the window to ${target} (position ${after.window.x},${after.window.y}); Wayland and tiling compositors may refuse to place windows`);
  }
  return target;
}

export interface ProjectSummary {
  name: string;
  path: string;
  dir?: string;
  modified?: number;
  exists?: boolean;
  audio?: string;
}

export interface StageInputOutput {
  name?: string;
  path: string;
  kind?: string;
  exists?: boolean;
}

export interface TranscriptWord {
  word: string;
  start: number;
  end: number;
}

export interface TranscriptLine {
  index?: number;
  text: string;
  start?: number;
  end?: number;
}

export interface StageInfo {
  id: string;
  title?: string;
  done?: boolean;
  optional?: boolean;
  /** From `workshop_status`: `ready`, `needs-work` or `failed`. */
  state?: string;
  inputs?: StageInputOutput[];
  outputs?: StageInputOutput[];
  why?: string;
  /** Animated subjects listed by the `animate` stage. */
  assets?: Array<{ id?: string; name?: string; path?: string }>;
  document?: {
    words?: TranscriptWord[];
    lines?: TranscriptLine[];
    text?: string;
  };
}

/** One footer workflow stage from `status.prerequisites` in `project.snapshot`. */
export interface StagePrerequisite {
  id?: string;
  state?: string;
  detail?: string;
}

export interface StoryboardLink {
  page: number;
  to_page: number;
  rect: [number, number, number, number];
}

export interface StoryboardIssue { page?: number; kind: string; severity: string; message: string; item?: number }
/** One slot's state from `storyboard.check`: `empty` is an image slot nothing requires. */
export interface StoryboardSlotStatus { status: "incomplete" | "review" | "complete" | "empty"; issues: string[] }
export interface StoryboardUntraced {
  page?: number;
  label?: string;
  name?: string;
  path?: string;
  link?: string;
  pages?: number[];
  fix?: string;
  reason?: string;
}
export interface StoryboardCheckPage {
  page: number; archetype: string; section?: string | null; status?: "incomplete" | "review" | "complete";
  unfilled: Array<{ label: string; text?: string; fix: string }>;
  issues?: StoryboardIssue[];
  slots?: Record<string, StoryboardSlotStatus>;
  /** Issue #122: image links with no matching project asset (once `storyboard.check` reports them). */
  untraced?: StoryboardUntraced[] | number;
}
/** A labelled frame from `storyboard.pages`; `bounds` are in the page's own `bounds` space. */
export interface StoryboardSlot {
  id: number; label: string; kind: "text" | "image" | "other";
  group: string; display: string; editable: boolean; locked?: boolean;
  text?: string | null; image?: string | null;
  bounds?: [number, number, number, number] | null; hidden: boolean; empty: boolean;
}
export interface StoryboardPage {
  page: number; archetype: string; section?: string | null; title: string;
  bounds: [number, number, number, number]; groups: string[]; slots: StoryboardSlot[];
}
export interface StoryboardData {
  path?: string; complete?: boolean; status?: string; page_count?: number; count?: number;
  pages?: StoryboardCheckPage[]; error?: string;
  /** Issue #122: top-level report of image links with no project asset (none is invented here). */
  untraced?: StoryboardUntraced[] | number;
  preflight?: { errors: number; issues: StoryboardIssue[] };
}

export interface ProjectAsset {
  id: string;
  name: string;
  kind?: string;
  /** From `project.list_assets`: the project folder the file sits in. */
  role?: "background" | "character" | "image";
  path?: string;
  character?: string;
  preview?: string;
}

export interface TrunkFile {
  name: string;
  path: string;
  rel_dir: string;
  group: string;
  group_label: string;
  group_order: number;
  craft_priority: number;
  badge: string;
  open_label: string;
  is_craft_project: boolean;
  is_image: boolean;
  visual_kind: string;
  current_timeline: boolean;
  can_animate: boolean;
  assets: ProjectAsset[];
  size_bytes: number;
  modified_ms: number;
  detail: string;
}

export interface TrunkInventory {
  ok?: boolean;
  project_dir?: string;
  count?: number;
  registered_asset_count?: number;
  files?: TrunkFile[];
  external_assets?: ProjectAsset[];
  missing_assets?: ProjectAsset[];
  error?: string;
}

export interface WorkshopData {
  project?: string;
  name?: string;
  status?: {
    stages?: StageInfo[];
    /** Footer stage states from `workshop_status`; an object, with an array tolerated for older payloads. */
    prerequisites?: Record<string, StagePrerequisite> | StagePrerequisite[];
    complete?: boolean;
    storyboard_complete?: boolean;
    film_timeline?: { exists?: boolean; path?: string };
    asset_files?: StageInputOutput[];
  };
  storyboard?: StoryboardData;
  assets?: {
    assets?: ProjectAsset[];
  };
  trunk?: TrunkInventory;
  characters?: CharacterSummary[];
}

export interface WordsWordItem {
  word_id: string;
  line_id?: string;
  sung_id?: string;
  text: string;
  start?: number;
  end?: number;
  action?: string;
  emphasis?: boolean;
  color?: string;
}

export interface WordsLineItem {
  line_id?: string;
  text: string;
  start?: number;
  end?: number;
  words?: WordsWordItem[];
}

export interface WordsStyle {
  font?: string;
  size?: number;
  color?: string;
  align?: string;
  layout?: string;
  entrance?: string;
  x?: number;
  y?: number;
}

export interface WordsAnimation {
  animation_id: string;
  title?: string;
  section?: string;
  source_revision?: number;
  used_in_video?: boolean;
  start?: number;
  end?: number;
  style?: WordsStyle;
  lines?: WordsLineItem[];
  words?: WordsWordItem[];
  scope?: { section?: string; occurrence?: number };
  design?: Record<string, { items?: LyricDesignItem[]; draws?: unknown[] }>;
  kinetic?: { style?: WordsStyle; layout?: string; entrance?: string; [key: string]: unknown };
}

export interface LyricDesignItem {
  text: string; x: number; y: number; size?: number; color?: string; font?: string;
  weight?: number; italic?: boolean; rotate?: number; outline?: string;
  word_ids?: string[]; motion_keys?: number[][]; [key: string]: unknown;
}

export interface WordsReadResponse {
  kinetic_library?: Record<string, Record<string, { name: string }>>;
  document?: { animations?: WordsAnimation[] };
  in_video?: string[];
  fonts?: Array<{ family: string; path: string; weight?: string }>;
  audio?: string;
  recipe_path?: string;
}

export type AnimationStyle = "sprite_2d" | "cutout_2_5d" | "rig_3d" | "cutout";

export interface Character3DClip {
  id: string;
  name?: string;
  animation_style?: AnimationStyle;
  source?: string;
  category?: string;
  duration?: number;
  in_place?: boolean;
  fbx?: string;
  glb?: string;
  blend?: string;
  preview?: string;
}

export interface Character3D {
  available?: boolean;
  animation_style?: AnimationStyle;
  primary_glb?: string;
  glb_files?: string[];
  blend_files?: string[];
  fbx_files?: string[];
  model_clips?: string[];
  clips?: Character3DClip[];
  rigged?: boolean;
  bone_count?: number;
  video_previews?: string[];
}

export interface CharacterSpriteSummary {
  name: string;
  animation_style?: AnimationStyle;
  ecproj: string;
  preview?: string;
  frames?: string[];
  fps?: number;
  duration?: number;
}

export interface CharacterViewSummary {
  view: string;
  animation_style?: AnimationStyle;
  drawing?: string;
  size?: [number, number];
  status?: string;
  parts?: CharacterPart[];
  actions?: Record<string, { start: number; end: number; times?: number }>;
  marked?: boolean;
  cut?: boolean;
  rigged?: boolean;
  ecproj?: string;
  psd?: boolean;
  revision?: number;
}

export interface CharacterSummary {
  tripo_3d?: Character3D;
  id: string;
  name?: string;
  animation_style?: AnimationStyle;
  animation_styles?: AnimationStyle[];
  views?: Array<string | CharacterViewSummary>;
  sprites?: Array<string | CharacterSpriteSummary>;
  shots?: string[];
  ecproj?: string;
  pcraft?: string;
  image?: string;
}

export interface CharacterPart {
  id: string;
  name?: string;
  parent?: string | null;
  visible?: boolean;
  joint?: [number, number];
  outline?: Array<[number, number]>;
  bounds?: [number, number, number, number];
  pins?: Array<[number, number]>;
  source?: string;
  file?: string;
}

export interface CharacterActionDef {
  id: string;
  views?: string[];
  duration?: number;
  loop?: boolean;
  model?: { clips?: string[]; yaw?: number };
}

export interface CharacterMetaResponse {
  tripo_3d?: Character3D;
  character?: string;
  animation_style?: AnimationStyle;
  animation_styles?: AnimationStyle[];
  skeleton?: string;
  views?: CharacterViewSummary[];
  parts?: CharacterPart[];
  actions?: CharacterActionDef[];
  sprites?: CharacterSpriteSummary[];
  shots?: Array<{
    name?: string;
    path: string;
    duration?: number;
  }>;
}

export interface AnimationRevisionDraft {
  trim_start: number;
  trim_end: number;
  speed: number;
  loop: boolean;
  in_place: boolean;
  notes: string;
  status: string;
}

export interface AnimationReviewModel {
  character: string;
  clip: string;
  path?: string;
  preview?: string;
  duration?: number;
  draft?: AnimationRevisionDraft;
}

export interface AnimationReviewResponse {
  review?: {
    revision?: number;
    models?: AnimationReviewModel[];
  };
}

export interface FilmFileStatus {
  path: string;
  exists: boolean;
  modified?: number;
}

export interface FilmStatusResponse {
  timeline: FilmFileStatus;
  preview: FilmFileStatus;
  export?: FilmFileStatus;
  preview_stale?: boolean;
  storyboard_stale?: boolean;
}

export interface TimelineClip {
  id?: string;
  name: string;
  start: number;
  end: number;
  enabled?: boolean;
  path?: string;
}

export interface TimelineTrack {
  id?: string;
  label: string;
  kind: "video" | "audio" | string;
  /** From `video.timeline`: the track's place in the film layout. */
  role?: "background" | "character" | "lyrics" | "song" | null;
  clips: TimelineClip[];
}

export interface TimelineResponse {
  duration: number;
  tracks: TimelineTrack[];
}

// Long-task notification set (#113): only the heavy producers notify, never scrub-time queries.
const NOTIFY_IDS = new Set(["video.assemble", "video.export", "image.generate"]);
const NOTIFY_PREFIXES = ["scribbles."];

let longTaskLabels: Map<string, string> | null = null;
async function notifyForTask(id: string, done: Promise<unknown>): Promise<void> {
  if (!NOTIFY_IDS.has(id) && !NOTIFY_PREFIXES.some((prefix) => id.startsWith(prefix))) return;
  if (!longTaskLabels) {
    try {
      const listed = await invoke<{ commands?: Array<{ id: string; label?: string; long_running?: boolean }> }>("execute_command", { id: "command.list", params: {} });
      longTaskLabels = new Map((listed.commands ?? []).filter((c) => c.long_running).map((c) => [c.id, c.label || c.id]));
    } catch {
      longTaskLabels = new Map();
    }
  }
  const label = longTaskLabels.get(id);
  if (!label) return;
  done.then(
    () => void notifyTaskComplete(label, true).catch(() => {}),
    (error) => void notifyTaskComplete(label, false, String(error)).catch(() => {}),
  );
}

export async function executeCommand<T = unknown>(
  id: string,
  params: Record<string, unknown> = {},
): Promise<T> {
  const done = invoke<T>("execute_command", { id, params });
  void notifyForTask(id, done);
  return done;
}

export function fileUrl(filePath?: string | null): string {
  if (!filePath) return "";
  return convertFileSrc(filePath);
}

export async function pickFile(options: {
  title: string;
  extensions?: string[];
  directory?: boolean;
  multiple?: boolean;
}): Promise<string | string[] | null> {
  return invoke("pick_file", { options });
}
