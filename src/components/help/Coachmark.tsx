import { t } from "@/i18n";
import { useI18n } from "@/contexts/PreferencesContext";
import { useCallback, useEffect, useState } from "react";
import { useLocation, useNavigate } from "react-router-dom";
import { getTour, type TourState } from "@/lib/tours";

const PADDING = 8;

/**
 * Highlights a real element of the current screen: dims the rest, frames the
 * element, points at it and explains it. Started through router state { tour }.
 * Collapsed <details> around the element are opened first.
 */
export function Coachmark() {
  useI18n();
  const location = useLocation();
  const navigate = useNavigate();
  const state = location.state as Partial<TourState> | null;
  const tour = getTour(state?.tour);
  const override = state?.target;
  const [rect, setRect] = useState<DOMRect | null>(null);

  const close = useCallback(() => {
    setRect(null);
    navigate(location.pathname, { replace: true, state: null });
  }, [navigate, location.pathname]);

  useEffect(() => {
    if (!tour) { setRect(null); return; }
    const targets = [override ?? tour.target, tour.fallback].filter((value): value is string => !!value);
    let element: Element | null = null;
    let attempts = 0;
    const measure = () => element && setRect(element.getBoundingClientRect());
    // The screen may still be loading data: wait for the element a little.
    const find = window.setInterval(() => {
      for (const target of targets) {
        element = document.querySelector(`[data-tour="${target}"]`);
        if (element) break;
      }
      if (element || ++attempts > 30) {
        window.clearInterval(find);
        if (!element) { close(); return; }
        for (let parent = element.parentElement; parent; parent = parent.parentElement) {
          if (parent instanceof HTMLDetailsElement) parent.open = true;
        }
        element.scrollIntoView({ block: "center", behavior: "smooth" });
        window.setTimeout(measure, 450);
      }
    }, 100);
    window.addEventListener("resize", measure);
    window.addEventListener("scroll", measure, true);
    return () => {
      window.clearInterval(find);
      window.removeEventListener("resize", measure);
      window.removeEventListener("scroll", measure, true);
    };
  }, [tour, override, close]);

  useEffect(() => {
    if (!rect) return;
    const onKey = (event: KeyboardEvent) => { if (event.key === "Escape") close(); };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [rect, close]);

  if (!tour || !rect) return null;

  const box = { top: rect.top - PADDING, left: rect.left - PADDING, width: rect.width + PADDING * 2, height: rect.height + PADDING * 2 };
  // Explanation on the side with more room; over the bottom of a tall element.
  const spaceAbove = box.top;
  const spaceBelow = window.innerHeight - (box.top + box.height);
  const placement = Math.max(spaceAbove, spaceBelow) < 180 ? "overlay" : spaceBelow >= spaceAbove ? "below" : "above";
  const position = placement === "below" ? { top: box.top + box.height + 8 } : placement === "above" ? { bottom: window.innerHeight - box.top + 8 } : { bottom: 16 };
  const hintLeft = Math.min(Math.max(box.left, 16), Math.max(16, window.innerWidth - 16 - 420));

  return <div className="fixed inset-0 z-50" onClick={close} role="dialog" aria-modal="true" aria-label={t(tour.title)}>
    <div className="pointer-events-none absolute rounded-xl shadow-[0_0_0_9999px_rgba(0,0,0,0.6)]" style={box}>
      <div className="absolute inset-0 animate-pulse rounded-xl ring-4 ring-amber-400" />
    </div>
    <div className="absolute flex w-[min(420px,calc(100vw-32px))] flex-col items-start gap-1" style={{ ...position, left: hintLeft }}>
      {placement === "below" && <span className="ml-6 text-3xl leading-none text-amber-400 drop-shadow">▲</span>}
      <div className="w-full rounded-xl border bg-card p-4 text-card-foreground shadow-xl" onClick={event => event.stopPropagation()}>
        <p className="text-sm font-semibold">{t(tour.title)}</p>
        <p className="mt-1 text-sm leading-snug">{t(tour.hint)}</p>
        <button type="button" onClick={close} autoFocus className="mt-3 w-full rounded-md bg-primary py-2 text-sm font-medium text-primary-foreground">{t("helpGotIt")}</button>
      </div>
      {placement === "above" && <span className="ml-6 text-3xl leading-none text-amber-400 drop-shadow">▼</span>}
    </div>
  </div>;
}
