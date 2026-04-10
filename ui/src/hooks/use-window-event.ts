import { useEffect, useEffectEvent } from "react";

export function useWindowEvent(eventName: string, handler: (event: Event) => void) {
  const onEvent = useEffectEvent(handler);

  useEffect(() => {
    const listener = (event: Event) => onEvent(event);
    window.addEventListener(eventName, listener as EventListener);
    return () => {
      window.removeEventListener(eventName, listener as EventListener);
    };
  }, [eventName, onEvent]);
}
