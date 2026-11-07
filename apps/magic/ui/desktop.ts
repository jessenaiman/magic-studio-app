import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface Request { id: number; method: string; params: Record<string, unknown> }

function surfaces(root = document, x = 0, y = 0): Array<{ doc: Document; x: number; y: number }> {
  const found = [{ doc: root, x, y }];
  for (const frame of root.querySelectorAll<HTMLIFrameElement>("iframe")) {
    if (!frame.getClientRects().length) continue;
    try { const child = frame.contentDocument; if (child) { const rect = frame.getBoundingClientRect(); found.push(...surfaces(child, x + rect.x, y + rect.y)); } } catch { /* External video players retain their own origin. */ }
  }
  return found;
}
function findElement(id: string): HTMLElement | null {
  for (const { doc } of surfaces()) { const element = doc.getElementById(id); if (element) return element; }
  return null;
}
function isForm(element: Element | null): element is HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement {
  return !!element && /^(INPUT|SELECT|TEXTAREA)$/.test(element.tagName);
}

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
            alerts: surfaces().flatMap(({ doc }) => Array.from(doc.querySelectorAll('[role="alert"]')).map(element => element.textContent?.trim()).filter(Boolean)),
            workspaces: surfaces().filter(surface => surface.doc !== document).map(({ doc }) => {
              const feedback = (doc.defaultView as unknown as { acFeedback?: { snapshot: () => Record<string, unknown> } })?.acFeedback?.snapshot();
              return { title: doc.title, feedback: feedback && {
                rendered: feedback.rendered, phase: feedback.phase, project: feedback.project, view: feedback.view,
                errors: feedback.errors, pending_invokes: feedback.pending_invokes,
                draft_fields: feedback.draft_fields, last_frontend_command: feedback.last_frontend_command, theatre: feedback.theatre,
              } };
            }) }; break;
        case "ui.elements": {
          const elements = surfaces().flatMap(({ doc, x, y }) => Array.from(doc.querySelectorAll<HTMLElement>("[id]"))
            .filter(el => el.getClientRects().length > 0 && (!params.prefix || el.id.startsWith(String(params.prefix))))
            .map(el => { const r = el.getBoundingClientRect(); return {
              id: el.id, label: el.getAttribute("aria-label") || el.textContent?.trim().slice(0, 120),
              tag: el.tagName.toLowerCase(), enabled: !el.matches(":disabled"),
              value: isForm(el) ? el.value : undefined,
              data: { ...el.dataset },
              rect: [r.x + x, r.y + y, r.width, r.height],
            }; }));
          result = { elements }; break;
        }
        case "ui.set": case "ui.set_view": {
          if (typeof params.id === "string") {
            if (Object.keys(params).some(key => !["id", "value"].includes(key))) throw new Error("Form changes accept id and value only");
            const element = findElement(params.id);
            if (!isForm(element)
              || !element.getClientRects().length || element.disabled) throw new Error("Form control is missing, hidden, or disabled");
            if (element.tagName !== "SELECT" && (element as HTMLInputElement).readOnly) throw new Error("Control is read-only");
            if (element.tagName === "INPUT" && ["file", "password", "checkbox", "radio"].includes(element.type)) throw new Error("This control type is not supported by ui.set");
            if (typeof params.value !== "string" && typeof params.value !== "number") throw new Error("value must be a string or number");
            const value = String(params.value);
            if (element.tagName === "SELECT" && !Array.from((element as HTMLSelectElement).options).some(option => option.value === value && !option.disabled)) throw new Error("Unknown or disabled option");
            const candidate = element.cloneNode(true) as typeof element;
            candidate.value = value;
            if (!candidate.checkValidity() || candidate.value !== value) throw new Error("Value is outside the control's accepted range");
            const setter = Object.getOwnPropertyDescriptor(Object.getPrototypeOf(element), "value")?.set;
            setter?.call(element, value);
            element.dispatchEvent(new Event(element.tagName === "SELECT" ? "change" : "input", { bubbles: true }));
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
          const element = findElement(String(params.id));
          if (!element || !element.getClientRects().length || element.matches(":disabled")) throw new Error("Control is missing, hidden, or disabled");
          if (method === "ui.click") {
            if (typeof element.click === "function") element.click();
            else element.dispatchEvent(new MouseEvent("click", { bubbles: true }));
          } else element.focus();
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
