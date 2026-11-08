import React, { useCallback, useEffect, useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";

// Desktop-style tooltip (the ArtCraft ui-tooltip pattern, without its dependencies):
// hover delay, viewport edge clamping, and a body portal so the app bar's blur and
// drag region cannot trap it. Icon-only controls get this instead of the native
// `title` attribute, which WebKitGTK shows late, unstyled and doubled.
interface TooltipProps {
  label: string;
  children: React.ReactElement<Record<string, unknown>>;
  position?: "top" | "bottom";
  delay?: number;
}

let tooltipSeq = 0;

export const Tooltip: React.FC<TooltipProps> = ({ label, children, position = "bottom", delay = 300 }) => {
  const [open, setOpen] = useState(false);
  const [coords, setCoords] = useState<{ x: number; y: number; ready: boolean }>({ x: 0, y: 0, ready: false });
  const triggerRef = useRef<HTMLSpanElement>(null);
  const tipRef = useRef<HTMLDivElement>(null);
  const timer = useRef<number | undefined>(undefined);
  const id = useRef(`magic-tip-${++tooltipSeq}`).current;

  const show = useCallback(() => {
    window.clearTimeout(timer.current);
    timer.current = window.setTimeout(() => setOpen(true), delay);
  }, [delay]);
  const hide = useCallback(() => {
    window.clearTimeout(timer.current);
    setOpen(false);
    setCoords((current) => ({ ...current, ready: false }));
  }, []);

  useLayoutEffect(() => {
    if (!open) return;
    const place = () => {
      const trigger = triggerRef.current?.getBoundingClientRect();
      if (!trigger) return;
      const tip = tipRef.current?.getBoundingClientRect();
      const width = tip?.width ?? 0;
      const height = tip?.height ?? 0;
      const pad = 8;
      const x = Math.max(pad, Math.min(trigger.left + trigger.width / 2 - width / 2, window.innerWidth - width - pad));
      const y = position === "bottom" ? trigger.bottom + 8 : trigger.top - height - 8;
      setCoords({ x, y, ready: true });
    };
    place();
    window.addEventListener("scroll", place, true);
    window.addEventListener("resize", place);
    return () => {
      window.removeEventListener("scroll", place, true);
      window.removeEventListener("resize", place);
    };
  }, [open, position]);

  // Stuck-tooltip guard: close on blur, tab hide and unmount.
  useEffect(() => {
    if (!open) return;
    window.addEventListener("blur", hide);
    document.addEventListener("visibilitychange", hide);
    return () => {
      window.removeEventListener("blur", hide);
      document.removeEventListener("visibilitychange", hide);
    };
  }, [open, hide]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  const trigger = React.cloneElement(children, { "aria-describedby": open ? id : undefined });

  return (
    <span
      ref={triggerRef}
      style={{ position: "relative", display: "inline-flex" }}
      onPointerEnter={show}
      onPointerLeave={hide}
      onPointerDown={hide}
      onFocusCapture={(event) => {
        if ((event.target as HTMLElement).matches(":focus-visible")) show();
      }}
      onBlurCapture={hide}
    >
      {trigger}
      {open &&
        createPortal(
          <div
            id={id}
            role="tooltip"
            ref={tipRef}
            style={{
              position: "fixed",
              left: coords.x,
              top: coords.y,
              opacity: coords.ready ? 1 : 0,
              transition: "opacity 150ms ease-out",
              zIndex: 1000,
              pointerEvents: "none",
              padding: "4px 8px",
              borderRadius: 3,
              border: "1px solid var(--border, #2a313c)",
              background: "var(--surface-2, #181c22)",
              color: "var(--text, #e6edf3)",
              font: "500 11px/1.4 ui-monospace, SFMono-Regular, monospace",
              letterSpacing: "0.1em",
              textTransform: "uppercase",
              whiteSpace: "nowrap",
            }}
          >
            {label}
          </div>,
          document.body,
        )}
    </span>
  );
};

export default Tooltip;
