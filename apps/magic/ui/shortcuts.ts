// GlobalShortcut helper — configurable opt-in shortcuts (Plugin 4).
// Mirrors craft-keymap: overrides stored through craft-settings (config.toml); no Super/Tab conflicts.
import { isRegistered, register, unregister, unregisterAll } from "@tauri-apps/plugin-global-shortcut";
import { modifierLabel } from "./os-info";

export type ShortcutId = "bring-to-front" | "review-design" | "review-motion";

// Default set avoids system shortcuts (no Super, no Ctrl+Tab/Delete).
export function defaultShortcuts(): Record<ShortcutId, string> {
  const m = modifierLabel("linux") === "Cmd" ? "Command" : "Control";
  // platform() is available via getOsInfo; defaults use OS-aware modifier label via modifierLabel in caller.
  return {
    "bring-to-front": `${m}+Shift+M`,
    "review-design": `${m}+Shift+D`,
    "review-motion": `${m}+Shift+O`,
  };
}

export async function isShortcutRegistered(shortcut: string): Promise<boolean> {
  return await isRegistered(shortcut);
}

export async function registerShortcut(shortcut: string, handler: () => void): Promise<void> {
  await register(shortcut, handler);
}

export async function unregisterShortcut(shortcut: string): Promise<void> {
  await unregister(shortcut);
}

export async function unregisterAllShortcuts(): Promise<void> {
  await unregisterAll();
}
