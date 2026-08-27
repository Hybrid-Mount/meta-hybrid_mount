// SPDX-License-Identifier: Apache-2.0

import { computed, onBeforeUnmount, ref, watch, type Ref } from "vue";

const EDGE_RESISTANCE = 3;
const SWIPE_THRESHOLD_RATIO = 0.22;
const SWIPE_THRESHOLD_MAX = 96;
const SWIPE_THRESHOLD_FALLBACK = 64;
const HORIZONTAL_LOCK_DISTANCE = 6;
const VERTICAL_LOCK_DISTANCE = 12;
const HORIZONTAL_INTENT_RATIO = 1.05;
const VERTICAL_INTENT_RATIO = 1.25;

export function useSwipePager(
  activeIndex: () => number,
  pageCount: () => number,
  selectPage: (index: number) => void,
  containerRef: Ref<HTMLElement | null>,
) {
  const dragOffset = ref(0);
  const isDragging = ref(false);
  const visitedPages = ref(new Set<number>());

  let pointerStartX = 0;
  let pointerStartY = 0;
  let activePointerId: number | null = null;
  let pendingOffset = 0;
  let animationFrame = 0;
  let direction: "horizontal" | "vertical" | null = null;

  const trackStyle = computed<Record<string, string>>(() => {
    const count = Math.max(pageCount(), 1);
    const index = Math.min(Math.max(activeIndex(), 0), count - 1);
    return {
      "--swipe-tab-count": String(count),
      "--swipe-base-translate": `${index * -(100 / count)}%`,
      "--swipe-drag-offset": `${dragOffset.value}px`,
    };
  });

  function visitRange(from: number, to: number): void {
    const count = pageCount();
    if (count <= 0) return;

    const first = Math.max(0, Math.min(from, to));
    const last = Math.min(count - 1, Math.max(from, to));
    const next = new Set(visitedPages.value);
    for (let index = first; index <= last; index += 1) next.add(index);
    visitedPages.value = next;
  }

  watch(activeIndex, (next, previous) => visitRange(previous ?? next, next), {
    flush: "sync",
    immediate: true,
  });

  function onPointerDown(event: PointerEvent): void {
    if (!event.isPrimary || (event.pointerType === "mouse" && event.button !== 0)) return;

    activePointerId = event.pointerId;
    pointerStartX = event.clientX;
    pointerStartY = event.clientY;
    pendingOffset = 0;
    dragOffset.value = 0;
    direction = null;
    isDragging.value = true;
    visitRange(activeIndex() - 1, activeIndex() + 1);

    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
      animationFrame = 0;
    }
  }

  function releasePointer(element: EventTarget | null): void {
    if (!(element instanceof HTMLElement) || activePointerId === null) return;
    if (element.hasPointerCapture(activePointerId)) {
      element.releasePointerCapture(activePointerId);
    }
  }

  function onPointerMove(event: PointerEvent): void {
    if (!isDragging.value || event.pointerId !== activePointerId) return;

    let deltaX = event.clientX - pointerStartX;
    const deltaY = event.clientY - pointerStartY;

    if (direction === null) {
      const absX = Math.abs(deltaX);
      const absY = Math.abs(deltaY);

      if (absX >= HORIZONTAL_LOCK_DISTANCE && absX > absY * HORIZONTAL_INTENT_RATIO) {
        direction = "horizontal";
        (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
      } else if (absY >= VERTICAL_LOCK_DISTANCE && absY > absX * VERTICAL_INTENT_RATIO) {
        direction = "vertical";
      } else {
        return;
      }
    }

    if (direction === "vertical") {
      releasePointer(event.currentTarget);
      activePointerId = null;
      isDragging.value = false;
      dragOffset.value = 0;
      direction = null;
      return;
    }

    if (event.cancelable) event.preventDefault();

    const index = activeIndex();
    const count = pageCount();
    if ((index === 0 && deltaX > 0) || (index === count - 1 && deltaX < 0)) {
      deltaX /= EDGE_RESISTANCE;
    }
    pendingOffset = deltaX;

    if (!animationFrame) {
      animationFrame = requestAnimationFrame(() => {
        animationFrame = 0;
        if (isDragging.value) dragOffset.value = pendingOffset;
      });
    }
  }

  function finishSwipe(commit: boolean, element: EventTarget | null): void {
    if (!isDragging.value) return;
    isDragging.value = false;
    releasePointer(element);

    if (animationFrame) {
      cancelAnimationFrame(animationFrame);
      animationFrame = 0;
      dragOffset.value = pendingOffset;
    }

    if (commit && direction === "horizontal") {
      const width = containerRef.value?.clientWidth ?? 0;
      const threshold = width
        ? Math.min(width * SWIPE_THRESHOLD_RATIO, SWIPE_THRESHOLD_MAX)
        : SWIPE_THRESHOLD_FALLBACK;
      const current = activeIndex();
      let next = current;
      if (dragOffset.value < -threshold && current < pageCount() - 1) next += 1;
      if (dragOffset.value > threshold && current > 0) next -= 1;
      if (next !== current) selectPage(next);
    }

    dragOffset.value = 0;
    pendingOffset = 0;
    direction = null;
    activePointerId = null;
  }

  function onPointerUp(event: PointerEvent): void {
    if (event.pointerId !== activePointerId) return;
    finishSwipe(true, event.currentTarget);
  }

  function onPointerCancel(event: PointerEvent): void {
    if (event.pointerId !== activePointerId) return;
    finishSwipe(false, event.currentTarget);
  }

  onBeforeUnmount(() => {
    if (animationFrame) cancelAnimationFrame(animationFrame);
  });

  return {
    isDragging,
    trackStyle,
    visitedPages,
    onPointerDown,
    onPointerMove,
    onPointerUp,
    onPointerCancel,
  };
}
