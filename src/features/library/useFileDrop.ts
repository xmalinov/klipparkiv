import { useEffect, useRef, useState } from "react";
import { getCurrentWebview } from "@tauri-apps/api/webview";

/**
 * Subscribes the mounted screen to native file drops.
 *
 * Native drag and drop is required to get absolute paths; with
 * `dragDropEnabled` the webview no longer emits HTML5 drop events.
 */
export function useFileDrop({
  onDrop,
  disabled = false,
}: {
  onDrop: (paths: string[]) => void | Promise<void>;
  disabled?: boolean;
}): { dragging: boolean } {
  const [dragging, setDragging] = useState(false);
  const handlerRef = useRef(onDrop);
  const disabledRef = useRef(disabled);

  useEffect(() => {
    handlerRef.current = onDrop;
  }, [onDrop]);

  useEffect(() => {
    disabledRef.current = disabled;
    if (disabled) setDragging(false);
  }, [disabled]);

  useEffect(() => {
    let alive = true;
    let unlisten: (() => void) | undefined;

    try {
      const pending = getCurrentWebview().onDragDropEvent((event) => {
        const payload = event.payload;
        if (payload.type === "enter") {
          if (!disabledRef.current && payload.paths.length > 0) setDragging(true);
          return;
        }
        if (payload.type === "leave") {
          setDragging(false);
          return;
        }
        if (payload.type === "drop") {
          setDragging(false);
          if (disabledRef.current || payload.paths.length === 0) return;
          try {
            void Promise.resolve(handlerRef.current(payload.paths)).catch(() => {});
          } catch {
            // A screen-level handler must never break the listener.
          }
        }
      });
      pending
        .then((stop) => {
          if (alive) unlisten = stop;
          else stop();
        })
        .catch(() => {});
    } catch {
      // Not running inside Tauri (unit tests, plain browser preview).
    }

    return () => {
      alive = false;
      unlisten?.();
    };
  }, []);

  return { dragging };
}
