// Shared globals of the Tauri UI. The UI loads as classic scripts (see ui/index.html),
// so each module's top-level names and window.* members are declared here once.
// This file is type-only: it is not shipped inside the app bundle.

interface AcFeedback {
  phase: string;
  errors: Array<Record<string, unknown>>;
  pending: number;
  ack: number;
  last: unknown;
  actions: WeakMap<object, unknown>;
  action?: any;
  report(phase?: string | null, ack?: number | null, event?: Record<string, unknown>): Promise<void>;
  acknowledge(revision: number, project?: string | null): Promise<void>;
  [key: string]: any;
}

interface Window {
  acFeedback: AcFeedback;
  uiConfig: UiConfig;
  uiRegistry: any;
  companionBoxes: any;
  workshopDrawers: any;
  workspaceNavigation: any;
  applicationFileMenu: any;
  textContextMenu: any;
  studioMenu: (id: string) => Promise<void> | void;
  magicUI: any;
  magicCompanions: any;
  magicCompanionBoxes: any;
  magicCompanionPaint: () => void;
  magicBackstageTerminal: any;
  MagicBackstage: any;
  __TAURI__: any;
}

/** One registered element in components.json. */
interface UiConfigEntry {
  component?: string;
  look?: string;
  density?: string;
  scroll?: string;
  label?: string;
  description?: string;
  [key: string]: any;
}

/** components.json as app.js loads it. */
interface UiConfig {
  version: number;
  looks: Record<string, any>;
  densities: string[];
  elements: Record<string, UiConfigEntry>;
  [key: string]: any;
}

declare var acFeedback: AcFeedback;
declare function motionModeSwitch(): any;

interface Event { acAction?: any }

// Stepping stones while views move to typed classes: DOM lookups in the classic-script UI
// are written against the real markup, so queries and event targets are loosely typed.
interface ParentNode {
  querySelector<E extends Element = any>(selectors: string): E | null;
  querySelectorAll<E extends Element = any>(selectors: string): NodeListOf<E>;
}
interface EventTarget {
  closest(selectors: string): any;
  value: any;
  tagName: string;
  [key: string]: any;
}
