const STORAGE_KEY = "sidecar.sidebarWidth";
const DEFAULT_WIDTH = 260;
const MIN_WIDTH = 180;
const MAX_WIDTH = 640;
const MIN_TERMINAL_WIDTH = 320;

/**
 * Drag-to-resize for the sidebar's right edge, following the same
 * pointerdown/move/up shape as the accordion's divider drag
 * (src/accordion.ts, wireDividerDrag) but along the horizontal axis and
 * for a single pane rather than a pair sharing a fixed total.
 *
 * Width is clamped between MIN_WIDTH and whichever is smaller of MAX_WIDTH
 * or "window width minus MIN_TERMINAL_WIDTH", so a drag can never squeeze
 * the terminal pane away entirely. The chosen width is persisted to
 * localStorage and restored on the next launch, the same mechanism the
 * accordion would use for panel heights if it persisted them (it currently
 * doesn't: panelHeights in src/accordion.ts lives only in memory).
 */
export function initSidebarResize(sidebar: HTMLElement, handle: HTMLElement, onResize: () => void) {
  sidebar.style.width = `${clampWidth(readStoredWidth())}px`;

  handle.addEventListener("pointerdown", (start: PointerEvent) => {
    start.preventDefault();
    const startWidth = sidebar.getBoundingClientRect().width;
    const startX = start.clientX;

    handle.classList.add("dragging");
    document.body.classList.add("accordion-drag-active");

    const onMove = (move: PointerEvent) => {
      const dx = move.clientX - startX;
      sidebar.style.width = `${clampWidth(startWidth + dx)}px`;
      onResize();
    };

    const onUp = () => {
      handle.classList.remove("dragging");
      document.body.classList.remove("accordion-drag-active");
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      persistWidth(sidebar.getBoundingClientRect().width);
    };

    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
  });
}

function clampWidth(width: number): number {
  const maxAvailable = Math.max(MIN_WIDTH, window.innerWidth - MIN_TERMINAL_WIDTH);
  const max = Math.min(MAX_WIDTH, maxAvailable);
  return Math.min(Math.max(width, MIN_WIDTH), max);
}

function readStoredWidth(): number {
  try {
    const stored = Number(localStorage.getItem(STORAGE_KEY));
    return Number.isFinite(stored) && stored > 0 ? stored : DEFAULT_WIDTH;
  } catch (err) {
    console.error("failed to read stored sidebar width", err);
    return DEFAULT_WIDTH;
  }
}

function persistWidth(width: number) {
  try {
    localStorage.setItem(STORAGE_KEY, String(width));
  } catch (err) {
    console.error("failed to persist sidebar width", err);
  }
}
