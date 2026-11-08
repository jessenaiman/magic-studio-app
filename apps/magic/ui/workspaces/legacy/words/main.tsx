import { createRoot } from "react-dom/client";
import { WordMotionPanel } from "./WordMotionPanel";
import type { WordsAnimation } from "./legacy";

/**
 * Island adapter for the still-imperative Lyrics page.
 *
 * `lyrics-layout.js` builds the panel with `nativeWordControls(anim)` and prepends the returned
 * element, so the React tree is mounted into a host that function returns. The host is unmounted
 * when a later legacy render replaces the controls container.
 */
function nativeWordControls(anim: WordsAnimation): HTMLElement {
  const host = document.createElement("div");
  host.className = "word-motion-island";
  const root = createRoot(host);
  root.render(<WordMotionPanel anim={anim} />);
  const observer = new MutationObserver(() => {
    if (host.isConnected) return;
    observer.disconnect();
    root.unmount();
  });
  observer.observe(document.body, { childList: true, subtree: true });
  return host;
}

(window as unknown as { nativeWordControls: typeof nativeWordControls }).nativeWordControls = nativeWordControls;
