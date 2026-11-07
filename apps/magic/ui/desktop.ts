import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import * as input from "./control-input";
import { workspaceDrafts } from "./workspace-drafts";
import { mediaState } from "./control-media";
import { requestAuthoringTime } from "./authoring-navigation";
import { getOsInfo } from "./api";
import { notificationPermission } from "./notification";
import { allowKeys, findElement, surfacesOf as surfaces } from "./control-input";

interface Request { id: number; method: string; params: Record<string, unknown> }
interface Violation { directive: string; blocked: string; source: string; line: number; document: string }
interface UiError { message: string; source?: string; line?: number; stack?: string; workspace?: string }

const MAX_VIOLATIONS = 50;
const INTERACTIVE = 'button,a[href],input,select,textarea,summary,[role="button"],[role="tab"],[role="menuitem"],[role="switch"],[role="checkbox"],[tabindex]:not([tabindex="-1"])';
const violations: Violation[] = [];
const shellErrors: UiError[] = [];
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

window.addEventListener("error", event => {
  const message = String(event.message || event.error).slice(0, 300);
  if (shellErrors.length >= MAX_VIOLATIONS) shellErrors.shift();
  shellErrors.push({ message, source: String(event.filename || "shell").split("/").pop(), line: event.lineno || undefined });
  void invoke("record_ui_error", { message, source: "shell", control: null }).catch(() => {});
});
window.addEventListener("unhandledrejection", event => {
  const message = String((event.reason as { message?: string })?.message || event.reason).slice(0, 300);
  if (shellErrors.length >= MAX_VIOLATIONS) shellErrors.shift();
  shellErrors.push({ message, source: "shell-promise" });
  void invoke("record_ui_error", { message, source: "shell-promise", control: null }).catch(() => {});
});

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
        case "ui.drafts":
          allowKeys(params, "ui.drafts", []);
          result = workspaceDrafts(); break;
        case "ui.inspect": case "ui.state": {
          watchViolations();
          const alerts = surfaces().flatMap(({ doc }) => Array.from(doc.querySelectorAll('[role="alert"]')).map(element => element.textContent?.trim()).filter((s): s is string => Boolean(s)));
          const workspaces = Array.from(document.querySelectorAll<HTMLIFrameElement>('iframe[id^="restored-"]')).map(frame => {
            let doc: Document | null = null;
            try { doc = frame.contentDocument; } catch { /* cross-origin */ }
            const feedback = (doc?.defaultView as unknown as { acFeedback?: { snapshot: () => Record<string, unknown> } })?.acFeedback?.snapshot();
            const hostError = frame.closest<HTMLElement>("[data-workspace-error]")?.dataset.workspaceError;
            const errors = Array.isArray(feedback?.errors) ? [...feedback.errors] : [];
            if (hostError && !errors.some(e => (e as { message?: string })?.message === hostError)) errors.push({ message: hostError, source: "host" });
            return { title: frame.title || doc?.title || frame.id, visible: frame.getClientRects().length > 0, feedback: feedback ? {
              rendered: feedback.rendered, phase: hostError ? "error" : feedback.phase, project: feedback.project, view: feedback.view,
              errors, pending_invokes: feedback.pending_invokes,
              draft_fields: feedback.draft_fields, last_frontend_command: feedback.last_frontend_command, theatre: feedback.theatre, animation: feedback.animation, authoring: feedback.authoring,
            } : hostError ? { rendered: false, phase: "error", errors } : undefined };
          });
          const workspaceErrors = workspaces.flatMap(w => (w.feedback?.errors || []).map(err => ({ ...(err as Record<string, unknown>), workspace: w.title })));
          result = { ...(inspect() as Record<string, unknown>),
            viewport: { width: innerWidth, height: innerHeight, devicePixelRatio },
            focus: input.focusedElement(),
            errors: [...shellErrors, ...workspaceErrors],
            cspViolations: violations,
            os: await getOsInfo().catch(() => null),
            notification: await notificationPermission().catch(() => false),
            alerts,
            workspaces }; break;
        }
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
              media: mediaState(el),
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
          if (keys.some(k => !["view","time"].includes(k))) throw new Error("View navigation accepts view and optional authoring time");
          const aliases: Record<string, string> = { projects: "project", lyrics: "design" };
          const view = aliases[String(params.view)] || String(params.view);
          if (!["home", "project", "storyboard", "design", "motion", "gallery", "animation", "studio"].includes(view)) throw new Error("Unknown view");
          if (params.time !== undefined) {
            if (!["design","motion"].includes(view) || typeof params.time !== "number" || !Number.isFinite(params.time) || params.time < 0) throw new Error("Authoring time requires Lyrics or Motion and nonnegative seconds");
            const project=(inspect() as {project?:string}).project;
            if (!project) throw new Error("Open a project before selecting an authoring moment");
            requestAuthoringTime(project,params.time);
          }
          setView(view);
          if (params.time !== undefined) for (const frame of document.querySelectorAll("iframe")) frame.contentWindow?.postMessage({magicReviewTime:true},location.origin);
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
