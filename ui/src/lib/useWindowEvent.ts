import { useEffect, useRef } from "react";

export function useWindowEvent(eventName: string, handler: (event: Event) => void) {
  const handlerRef = useRef(handler);
  handlerRef.current = handler;

  useEffect(() => {
    const listener = (event: Event) => handlerRef.current(event);
    window.addEventListener(eventName, listener as EventListener);
    return () => {
      window.removeEventListener(eventName, listener as EventListener);
    };
  }, [eventName]);
}

