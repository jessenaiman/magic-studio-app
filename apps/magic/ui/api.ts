import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";
import { arch, eol, exeExtension, family, locale, platform, type as osTypeFn, version } from "@tauri-apps/plugin-os";
import { currentMonitor, getCurrentWindow } from "@tauri-apps/api/window";
import { formatOsInfo, type OsInfo } from "./os-info";
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
  inputs?: StageInputOutput[];
  outputs?: StageInputOutput[];
  why?: string;
  document?: {
    words?: TranscriptWord[];
    lines?: TranscriptLine[];
    text?: string;
  };
}

export interface StoryboardLink {
  page: number;
  to_page: number;
  rect: [number, number, number, number];
}

export interface StoryboardIssue { page?: number; kind: string; severity: string; message: string; item?: number }
export interface StoryboardCheckPage {
  page: number; archetype: string; status?: "incomplete" | "review" | "complete";
  unfilled: Array<{ label: string; text?: string; fix: string }>;
  issues?: StoryboardIssue[];
}
export interface StoryboardData {
  path?: string; complete?: boolean; status?: string; page_count?: number; count?: number;
  pages?: StoryboardCheckPage[]; error?: string;
  preflight?: { errors: number; issues: StoryboardIssue[] };
}

export interface ProjectAsset {
  id: string;
  name: string;
  kind?: string;
  path?: string;
  character?: string;
  preview?: string;
}

export interface WorkshopData {
  project?: string;
  name?: string;
  status?: {
    stages?: StageInfo[];
    complete?: boolean;
    storyboard_complete?: boolean;
    asset_files?: StageInputOutput[];
  };
  storyboard?: StoryboardData;
  assets?: {
    assets?: ProjectAsset[];
  };
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
  animations?: WordsAnimation[];
  audio?: string;
  recipe_path?: string;
}

export interface Character3D {
  primary_glb?: string; glb_files?: string[]; model_clips?: string[]; rigged?: boolean;
  video_previews?: string[];
}

export interface CharacterSummary {
  tripo_3d?: Character3D;
  id: string;
  name?: string;
  views?: string[];
  sprites?: string[];
  shots?: string[];
  ecproj?: string;
  pcraft?: string;
  image?: string;
}

export interface CharacterPart {
  id: string;
  name?: string;
  parent?: string;
  visible?: boolean;
  joint?: [number, number];
}

export interface CharacterMetaResponse {
  tripo_3d?: Character3D;
  character?: string;
  views?: Array<{
    view: string;
    parts?: CharacterPart[];
    actions?: string[];
    ecproj?: string;
  }>;
  parts?: CharacterPart[];
  actions?: string[];
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
  clips: TimelineClip[];
}

export interface TimelineResponse {
  duration: number;
  tracks: TimelineTrack[];
}

export async function executeCommand<T = unknown>(
  id: string,
  params: Record<string, unknown> = {},
): Promise<T> {
  return invoke<T>("execute_command", { id, params });
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
  return openFileDialog({
    title: options.title,
    directory: options.directory,
    multiple: options.multiple,
    filters: options.extensions
      ? [{ name: options.title, extensions: options.extensions }]
      : undefined,
  });
}
