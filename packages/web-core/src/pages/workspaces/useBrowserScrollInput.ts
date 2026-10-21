import { useEffect, useMemo, type RefObject } from 'react';
import type { BrowserAction, BrowserActionResult } from 'shared/types';

export interface FramePoint {
  x: number;
  y: number;
  scale: number;
  width: number;
  height: number;
}

type Pointer = React.PointerEvent<Element>;
type Scroll = Extract<BrowserAction, { type: 'scroll' }>;

export function useBrowserScrollInput({
  surfaceRef,
  enabled,
  context,
  toFrameCoords,
  sendInput,
}: {
  surfaceRef: RefObject<HTMLDivElement | null>;
  enabled: boolean;
  context: string;
  toFrameCoords: (x: number, y: number) => FramePoint | null;
  sendInput: (action: BrowserAction) => Promise<BrowserActionResult | null>;
}) {
  const input = useMemo(() => {
    let disposed = false;
    let animation: number | null = null;
    let pending: Scroll | null = null;
    let inFlight = false;
    const pointers = new Set<number>();
    let gesture: {
      id: number;
      startX: number;
      startY: number;
      lastX: number;
      lastY: number;
      anchor: FramePoint;
      scrolling: boolean;
    } | null = null;

    // One command in flight, one accumulated delta: slow connections cannot
    // build an unbounded input queue. Disposed contexts never replay input.
    const schedule = () => {
      if (disposed || inFlight || animation !== null || !pending) return;
      animation = requestAnimationFrame(() => {
        animation = null;
        if (disposed || !pending) return;
        const deltaX = Math.trunc(pending.delta_x);
        const deltaY = Math.trunc(pending.delta_y);
        if (deltaX === 0 && deltaY === 0) return;
        const action = { ...pending, delta_x: deltaX, delta_y: deltaY };
        pending.delta_x -= deltaX;
        pending.delta_y -= deltaY;
        if (pending.delta_x === 0 && pending.delta_y === 0) pending = null;
        inFlight = true;
        void sendInput(action)
          .catch(() => {})
          .finally(() => {
            inFlight = false;
            schedule();
          });
      });
    };
    const scroll = (point: FramePoint, dx: number, dy: number) => {
      if (!enabled || disposed || !Number.isFinite(dx) || !Number.isFinite(dy))
        return;
      const bound = (delta: number) => Math.max(-32768, Math.min(32768, delta));
      // Without the remote DOM we cannot know whether two points share a
      // scroller. Discard old pending movement when the hit point changes.
      if (pending && (pending.x !== point.x || pending.y !== point.y)) {
        pending = null;
      }
      pending = {
        type: 'scroll',
        x: point.x,
        y: point.y,
        delta_x: bound((pending?.delta_x ?? 0) + dx),
        delta_y: bound((pending?.delta_y ?? 0) + dy),
      };
      if (pending.delta_x === 0 && pending.delta_y === 0) pending = null;
      schedule();
    };
    const isTouch = (e: Pointer) =>
      e.pointerType === 'touch' || e.pointerType === 'pen';
    const move = (e: Pointer) => {
      if (!enabled || disposed || !gesture || gesture.id !== e.pointerId)
        return;
      const g = gesture;
      if (!g.scrolling) {
        if (Math.hypot(e.clientX - g.startX, e.clientY - g.startY) < 8) return;
        g.scrolling = true;
      }
      e.preventDefault();
      // Keep the original hit target for nested scrollers even when the finger
      // leaves the frame. The content follows the finger, opposite wheel deltas.
      scroll(
        g.anchor,
        (g.lastX - e.clientX) / g.anchor.scale,
        (g.lastY - e.clientY) / g.anchor.scale
      );
      g.lastX = e.clientX;
      g.lastY = e.clientY;
    };
    return {
      context,
      activate() {
        disposed = false;
      },
      dispose() {
        disposed = true;
        if (animation !== null) cancelAnimationFrame(animation);
        animation = null;
        pending = null;
        gesture = null;
        pointers.clear();
      },
      wheel(e: WheelEvent) {
        if (!enabled || disposed) return;
        const point = toFrameCoords(e.clientX, e.clientY);
        if (!point) return;
        e.preventDefault();
        // Ctrl+wheel is browser pinch/zoom, not remote scrolling.
        if (e.ctrlKey) return;
        const unitX =
          e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? point.width : 1;
        const unitY =
          e.deltaMode === 1 ? 16 : e.deltaMode === 2 ? point.height : 1;
        scroll(
          point,
          (e.deltaX * unitX) / point.scale,
          (e.deltaY * unitY) / point.scale
        );
      },
      pointerDown(e: Pointer) {
        if (!enabled || disposed || !isTouch(e)) return;
        e.currentTarget.setPointerCapture(e.pointerId);
        pointers.add(e.pointerId);
        if (pointers.size !== 1) {
          pending = null;
          gesture = null;
          return;
        }
        const anchor = toFrameCoords(e.clientX, e.clientY);
        if (!anchor) return;
        gesture = {
          id: e.pointerId,
          startX: e.clientX,
          startY: e.clientY,
          lastX: e.clientX,
          lastY: e.clientY,
          anchor,
          scrolling: false,
        };
      },
      pointerMove(e: Pointer) {
        if (!isTouch(e)) return false;
        move(e);
        return true;
      },
      // Return true when the caller should suppress its ordinary click handler.
      pointerUp(e: Pointer) {
        if (!isTouch(e)) return false;
        move(e);
        const tap =
          enabled &&
          !disposed &&
          gesture?.id === e.pointerId &&
          !gesture.scrolling;
        pointers.delete(e.pointerId);
        gesture = null;
        return !tap;
      },
      pointerCancel(e: Pointer) {
        // Normal capture release follows pointerup; keep its final delta.
        if (gesture?.id === e.pointerId) pending = null;
        pointers.delete(e.pointerId);
        gesture = null;
      },
    };
    // Context includes session, connection and control generation. Each gets a
    // separate queue so old acknowledgements cannot flush new-session work.
  }, [enabled, context, toFrameCoords, sendInput]);

  useEffect(() => {
    input.activate();
    const surface = surfaceRef.current;
    surface?.addEventListener('wheel', input.wheel, { passive: false });
    return () => {
      surface?.removeEventListener('wheel', input.wheel);
      input.dispose();
    };
  }, [input, surfaceRef]);
  return input;
}
