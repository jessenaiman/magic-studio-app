import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { open as openFileDialog } from "@tauri-apps/plugin-dialog";

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

// Translate the engine document once; never discard the authored design/kinetic data.
export function lyricSections(response: WordsReadResponse): WordsAnimation[] {
  return (response.document?.animations || []).map(animation => {
    const grouped = new Map<string, WordsLineItem>();
    for (const word of animation.words || []) {
      const id = word.line_id || word.sung_id || "line";
      const line = grouped.get(id) || { line_id: id, text: "", words: [] };
      line.words!.push(word);
      line.text = line.words!.map(item => item.text).join(" ");
      if (typeof word.start === "number") line.start = Math.min(line.start ?? word.start, word.start);
      if (typeof word.end === "number") line.end = Math.max(line.end ?? word.end, word.end);
      grouped.set(id, line);
    }
    return { ...animation, lines: [...grouped.values()],
      title: animation.title || animation.scope?.section?.replace(/_/g, " ") || animation.animation_id,
      style: { ...animation.kinetic?.style, ...animation.style },
      used_in_video: response.in_video?.includes(animation.animation_id) };
  });
}
