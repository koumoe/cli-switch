import { useEffect } from "react";

const SCROLLING_ATTR = "data-scrolling";
const SCROLLBAR_VISIBILITY_MS = 420;

function resolveScrollTarget(target: EventTarget | null): HTMLElement | null {
  if (target instanceof HTMLElement) {
    return target;
  }

  if (target instanceof Document) {
    const scrollingElement = target.scrollingElement;
    if (scrollingElement instanceof HTMLElement) {
      return scrollingElement;
    }

    return target.documentElement;
  }

  return null;
}

export function ScrollbarVisibilityController() {
  useEffect(() => {
    const timers = new Map<HTMLElement, number>();

    function markScrolling(element: HTMLElement) {
      element.setAttribute(SCROLLING_ATTR, "true");

      const activeTimer = timers.get(element);
      if (activeTimer) {
        window.clearTimeout(activeTimer);
      }

      const timer = window.setTimeout(() => {
        element.removeAttribute(SCROLLING_ATTR);
        timers.delete(element);
      }, SCROLLBAR_VISIBILITY_MS);

      timers.set(element, timer);
    }

    function handleScroll(event: Event) {
      const target = resolveScrollTarget(event.target);
      if (!target) {
        return;
      }

      markScrolling(target);
    }

    document.addEventListener("scroll", handleScroll, true);

    return () => {
      document.removeEventListener("scroll", handleScroll, true);
      timers.forEach((timer, element) => {
        window.clearTimeout(timer);
        element.removeAttribute(SCROLLING_ATTR);
      });
      timers.clear();
    };
  }, []);

  return null;
}
