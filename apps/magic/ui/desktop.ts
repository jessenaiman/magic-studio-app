import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Request { id: number; method: string; params: Record<string, unknown> }

// Uses real DOM controls; unsupported operations fail rather than reporting fake success.
export async function connectDesktop(
  inspect: () => unknown,
  setView: (view: string) => void,
  refresh: () => Promise<void>,
) {
  const unlistenUi = await listen<Request>("magic-ui-request", async ({ payload }) => {
    try {
      const { method, params } = payload;
      let result: unknown;
      switch (method) {
        case "ui.inspect": case "ui.state":
          result = inspect(); break;
        case "ui.elements": {
          const elements = Array.from(document.querySelectorAll<HTMLElement>("[id]"))
            .filter(el => el.getClientRects().length > 0 && (!params.prefix || el.id.startsWith(String(params.prefix))))
            .map(el => { const r = el.getBoundingClientRect(); return {
              id: el.id, label: el.getAttribute("aria-label") || el.textContent?.trim().slice(0, 120),
              tag: el.tagName.toLowerCase(), enabled: !el.matches(":disabled"),
              rect: [r.x, r.y, r.width, r.height],
            }; });
          result = { elements }; break;
        }
        case "ui.set": case "ui.set_view": {
          const keys = Object.keys(params);
          if (keys.some(k => k !== "view")) throw new Error("Only the view field is supported by ui.set");
          const aliases: Record<string, string> = { projects: "project", lyrics: "design" };
          const view = aliases[String(params.view)] || String(params.view);
          if (!["home", "project", "storyboard", "design", "animation", "studio"].includes(view)) throw new Error("Unknown view");
          setView(view);
          result = { ok: true, requestedView: view }; break;
        }
        case "ui.click": case "ui.focus": {
          if (Object.keys(params).some(key => key !== "id")) throw new Error("Only ID-based click/focus is implemented");
          const element = document.getElementById(String(params.id));
          if (!element || !element.getClientRects().length || element.matches(":disabled")) throw new Error("Control is missing, hidden, or disabled");
          if (method === "ui.click") element.click(); else element.focus();
          result = { ok: true, id: element.id }; break;
        }
        default: throw new Error(`Unsupported Tauri UI operation: ${method}`);
      }
      await invoke("ui_reply", { id: payload.id, result });
    } catch (error) {
      await invoke("ui_reply", { id: payload.id, error: String(error) });
    }
  });
  const unlistenSession = await listen("magic-session-changed", () => { void refresh().catch(console.error); });
  return () => { unlistenUi(); unlistenSession(); };
}
