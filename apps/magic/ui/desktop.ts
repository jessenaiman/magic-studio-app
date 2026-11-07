import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as input from "./control-input";
import { allowKeys, findElement, surfacesOf as surfaces } from "./control-input";

interface Request { id: number; method: string; params: Record<string, unknown> }
interface Violation { directive: string; blocked: string; source: string; line: number; document: string }

const MAX_VIOLATIONS = 50;
const INTERACTIVE = 'button,a[href],input,select,textarea,summary,[role="button"],[role="tab"],[role="menuitem"],[role="switch"],[role="checkbox"],[tabindex]:not([tabindex="-1"])';
const violations: Violation[] = [];
const watched = new WeakSet<Document>();
// A CSP that blocks something the app needs fails silently in the page; keep the evidence for ui.inspect.
function watchViolations() {
  for (const { doc } of surfaces()) {
    if (watched.has(doc)) continue;
    watched.add(doc);
    doc.addEventListener("securitypolicyviolation", event => {
      if (violations.length >= MAX_VIOLATIONS) violations.shift();
      violations.push({ directive: event.violatedDirective, blocked: event.blockedURI, source: event.sourceFile, line: event.lineNumber, document: doc.location.pathname });
    });
  }
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
  watchViolations();
  const unlistenUi = await listen<Request>("magic-ui-request", async ({ payload }) => {
    try {
      const { method, params } = payload;
      let result: unknown;
      switch (method) {
        case "ui.inspect": case "ui.state":
          watchViolations();
          result = { ...(inspect() as Record<string, unknown>),
            viewport: { width: innerWidth, height: innerHeight, devicePixelRatio },
            focus: input.focusedElement(),
            cspViolations: violations,
            alerts: surfaces().flatMap(({ doc }) => Array.from(doc.querySelectorAll('[role="alert"]')).map(element => element.textContent?.trim()).filter(Boolean)),
            workspaces: surfaces().filter(surface => surface.doc !== document).map(({ doc }) => {
              const feedback = (doc.defaultView as unknown as { acFeedback?: { snapshot: () => Record<string, unknown> } })?.acFeedback?.snapshot();
              return { title: doc.title, feedback: feedback && {
                rendered: feedback.rendered, phase: feedback.phase, project: feedback.project, view: feedback.view,
                errors: feedback.errors, pending_invokes: feedback.pending_invokes,
                draft_fields: feedback.draft_fields, last_frontend_command: feedback.last_frontend_command, theatre: feedback.theatre, animation: feedback.animation, authoring: feedback.authoring,
              } };
            }) }; break;
        case "ui.elements": {
          allowKeys(params, "ui.elements", ["prefix", "unidentified"]);
          if (params.unidentified === true) {
            // Craft rule: every interactive widget has an automation id. List the ones that do not.
            const unidentified = surfaces().flatMap(({ doc, x, y }) => Array.from(doc.querySelectorAll<HTMLElement>(INTERACTIVE))
              .filter(el => !el.id && el.getClientRects().length > 0)
              .map(el => { const r = el.getBoundingClientRect(); return {
                tag: el.tagName.toLowerCase(), type: (el as HTMLInputElement).type || undefined,
                label: el.getAttribute("aria-label") || el.getAttribute("title") || el.textContent?.trim().slice(0, 80) || el.getAttribute("placeholder") || undefined,
                class: el.className && typeof el.className === "string" ? el.className.slice(0, 80) : undefined,
                parent: el.closest("[id]")?.id, document: doc.location.pathname, rect: [r.x + x, r.y + y, r.width, r.height],
              }; }));
            result = { unidentified }; break;
          }
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
          if (typeof params.id === "string" && "checked" in params) { result = input.setChecked(params); break; }
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
            input.commitValueEvents(element);
            if (element.value !== value) throw new Error("The control did not accept the requested value");
            result = { ok: true, id: params.id, value: element.value }; break;
          }
          const keys = Object.keys(params);
          if (keys.some(k => k !== "view")) throw new Error("Only the view field is supported by ui.set");
          const aliases: Record<string, string> = { projects: "project", lyrics: "design" };
          const view = aliases[String(params.view)] || String(params.view);
          if (!["home", "project", "storyboard", "design", "motion", "gallery", "animation", "studio"].includes(view)) throw new Error("Unknown view");
          setView(view);
          result = { ok: true, requestedView: view }; break;
        }
        case "ui.click": result = input.click(params); break;
        case "ui.move": result = input.move(params); break;
        case "ui.drag": result = input.drag(params); break;
        case "ui.scroll": result = input.scroll(params); break;
        case "ui.key": result = input.key(params); break;
        case "ui.type": case "ui.text": result = input.type(params); break;
        case "ui.focus": {
          allowKeys(params, "ui.focus", ["id"]);
          const element = findElement(String(params.id));
          if (!element || !element.getClientRects().length || element.matches(":disabled")) throw new Error("Control is missing, hidden, or disabled");
          element.focus();
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
