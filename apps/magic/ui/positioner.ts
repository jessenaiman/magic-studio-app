// Window positioning through @tauri-apps/plugin-positioner (granted by `positioner:default` in
// capabilities/desktop.json). Positions are relative to the current screen.
import { moveWindow, moveWindowConstrained, Position } from "@tauri-apps/plugin-positioner";

export { moveWindow, moveWindowConstrained, Position };

export type Rect = { x: number; y: number; width: number; height: number };

// Pixel slack for window-manager borders and rounding.
const EDGE_TOLERANCE = 8;

const near = (a: number, b: number) => Math.abs(a - b) <= EDGE_TOLERANCE;

// `window` and `screen` are in the same physical-pixel space (outerPosition/outerSize, monitor position/size).
export function isTopRight(window: Rect, screen: Rect): boolean {
  return near(window.y, screen.y) && near(window.x + window.width, screen.x + screen.width);
}

export function isTopLeft(window: Rect, screen: Rect): boolean {
  return near(window.y, screen.y) && near(window.x, screen.x);
}

export function isBottomRight(window: Rect, screen: Rect): boolean {
  return near(window.y + window.height, screen.y + screen.height) && near(window.x + window.width, screen.x + screen.width);
}

export function isBottomLeft(window: Rect, screen: Rect): boolean {
  return near(window.y + window.height, screen.y + screen.height) && near(window.x, screen.x);
}

export function isCentered(window: Rect, screen: Rect): boolean {
  return near(window.x + window.width / 2, screen.x + screen.width / 2) && near(window.y + window.height / 2, screen.y + screen.height / 2);
}
