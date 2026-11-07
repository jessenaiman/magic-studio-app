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
          result = { ...(inspect() as Record<string, unknown>),
            alerts: Array.from(document.querySelectorAll('[role="alert"]')).map(element => element.textContent?.trim()).filter(Boolean) }; break;
        case "ui.elements": {
          const elements = Array.from(document.querySelectorAll<HTMLElement>("[id]"))
            .filter(el => el.getClientRects().length > 0 && (!params.prefix || el.id.startsWith(String(params.prefix))))
            .map(el => { const r = el.getBoundingClientRect(); return {
              id: el.id, label: el.getAttribute("aria-label") || el.textContent?.trim().slice(0, 120),
              tag: el.tagName.toLowerCase(), enabled: !el.matches(":disabled"),
              value: el instanceof HTMLInputElement || el instanceof HTMLSelectElement || el instanceof HTMLTextAreaElement ? el.value : undefined,
              data: { ...el.dataset },
              rect: [r.x, r.y, r.width, r.height],
            }; });
          result = { elements }; break;
        }
        case "ui.set": case "ui.set_view": {
          if (typeof params.id === "string") {
            if (Object.keys(params).some(key => !["id", "value"].includes(key))) throw new Error("Form changes accept id and value only");
            const element = document.getElementById(params.id);
            if (!(element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement || element instanceof HTMLSelectElement)
              || !element.getClientRects().length || element.disabled) throw new Error("Form control is missing, hidden, or disabled");
            if ((element instanceof HTMLInputElement || element instanceof HTMLTextAreaElement) && element.readOnly) throw new Error("Control is read-only");
            if (element instanceof HTMLInputElement && ["file", "password", "checkbox", "radio"].includes(element.type)) throw new Error("This control type is not supported by ui.set");
            if (typeof params.value !== "string" && typeof params.value !== "number") throw new Error("value must be a string or number");
            const value = String(params.value);
            if (element instanceof HTMLSelectElement && !Array.from(element.options).some(option => option.value === value && !option.disabled)) throw new Error("Unknown or disabled option");
            const candidate = element.cloneNode(true) as typeof element;
            candidate.value = value;
            if (!candidate.checkValidity() || candidate.value !== value) throw new Error("Value is outside the control's accepted range");
            const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")?.set;
            setter?.call(element, value);
            element.dispatchEvent(new Event(element instanceof HTMLSelectElement ? "change" : "input", { bubbles: true }));
            result = { ok: true, id: params.id, value: element.value }; break;
          }
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
