// GlobalShortcut helper — configurable opt-in shortcuts (Plugin 4).
// Uses Rust-side craft-keymap registration via Tauri commands.
// Rust registers all keymap bindings as system hotkeys on startup; this module
// provides TS access to re-register or query registration status.

import { invoke } from "@tauri-apps/api/core";

export type ShortcutId = "bring-to-front" | "review-design" | "review-motion";

// Default set avoids system shortcuts (no Super, no Ctrl+Tab/Delete).
// These are registered by Rust on startup from craft-keymap catalog.
export function defaultShortcuts(): Record<ShortcutId, string> {
  return {
    "bring-to-front": "Command+Shift+M",
    "review-design": "Command+Shift+D",
    "review-motion": "Command+Shift+O",
  };
}

export async function reRegisterGlobalShortcuts(): Promise<{ ok: boolean }> {
  return await invoke("global_shortcut::reregister_cmd");
}

export async function listRegisteredGlobalShortcuts(): Promise<{ registered: string[] } | { note: string }> {
  return await invoke("global_shortcut::list_registered_cmd");
}

// Legacy helpers — now delegate to Rust. Kept for API compatibility.
export async function isShortcutRegistered(_shortcut: string): Promise<boolean> {
  const result = await listRegisteredGlobalShortcuts();
  return "registered" in result && result.registered.length > 0;
}

export async function registerShortcut(_shortcut: string, _handler: () => void): Promise<void> {
  // Rust registers all shortcuts from keymap catalog on startup.
  // Individual registration is not supported; use reRegisterGlobalShortcuts() after keymap changes.
  await reRegisterGlobalShortcuts();
}

export async function unregisterShortcut(_shortcut: string): Promise<void> {
  await reRegisterGlobalShortcuts();
}

export async function unregisterAllShortcuts(): Promise<void> {
  await reRegisterGlobalShortcuts();
}