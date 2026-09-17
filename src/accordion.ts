export type PanelId = "files" | "git" | "issues";

const PANEL_IDS: PanelId[] = ["files", "git", "issues"];
const MIN_PANEL_HEIGHT = 60;
const DRAG_THRESHOLD = 5;

/**
 * The sidebar's Files/Git/Issues sections. Any subset can be open at once,
 * each sharing the available height (equally by default, or a size you've
 * dragged a divider to). Order is independent of open/closed state and only
 * changes when you drag a header; opening or closing a panel never moves it.
 */
export class Accordion {
  private container: HTMLElement;
  private items: Record<PanelId, HTMLElement>;
  private panelOrder: PanelId[] = [...PANEL_IDS];
  private openIds = new Set<PanelId>(["files"]);
  private panelHeights: Partial<Record<PanelId, number>> = {};
  private onOpen?: (id: PanelId) => void;

  constructor(container: HTMLElement, onOpen?: (id: PanelId) => void) {
    this.container = container;
    this.onOpen = onOpen;
    this.items = {
      files: container.querySelector('[data-panel="files"]')!,
      git: container.querySelector('[data-panel="git"]')!,
      issues: container.querySelector('[data-panel="issues"]')!,
    };

    for (const id of PANEL_IDS) {
      const header = this.items[id].querySelector<HTMLElement>(".accordion-header")!;
      this.wireHeaderDrag(header, id);
    }

    this.render();
  }

  private wireHeaderDrag(header: HTMLElement, id: PanelId) {
    header.addEventListener("pointerdown", (start: PointerEvent) => {
      let dragging = false;

      const onMove = (move: PointerEvent) => {
        if (!dragging && Math.hypot(move.clientX - start.clientX, move.clientY - start.clientY) > DRAG_THRESHOLD) {
          dragging = true;
          header.classList.add("dragging");
          document.body.classList.add("accordion-drag-active");
        }
        if (!dragging) return;

        const others = this.panelOrder.filter((p) => p !== id);
        let targetIndex = others.length;
        for (let i = 0; i < others.length; i++) {
          const otherHeader = this.items[others[i]].querySelector<HTMLElement>(".accordion-header")!;
          const rect = otherHeader.getBoundingClientRect();
          if (move.clientY < rect.top + rect.height / 2) {
            targetIndex = i;
            break;
          }
        }
        others.splice(targetIndex, 0, id);
        if (others.join() !== this.panelOrder.join()) {
          this.panelOrder = others;
          this.render();
        }
      };

      const onUp = () => {
        header.classList.remove("dragging");
        document.body.classList.remove("accordion-drag-active");
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
        if (!dragging) this.toggle(id);
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    });
  }

  private wireDividerDrag(divider: HTMLElement, a: PanelId, b: PanelId) {
    divider.addEventListener("pointerdown", (start: PointerEvent) => {
      start.preventDefault();
      const startAHeight = this.items[a].getBoundingClientRect().height;
      const startBHeight = this.items[b].getBoundingClientRect().height;
      const totalHeight = startAHeight + startBHeight;
      const minDy = MIN_PANEL_HEIGHT - startAHeight;
      const maxDy = startBHeight - MIN_PANEL_HEIGHT;

      divider.classList.add("dragging");
      document.body.classList.add("accordion-drag-active");

      const onMove = (move: PointerEvent) => {
        const dy = Math.min(Math.max(move.clientY - start.clientY, minDy), maxDy);
        const newA = startAHeight + dy;
        const newB = totalHeight - newA;
        this.panelHeights[a] = newA;
        this.panelHeights[b] = newB;
        this.items[a].style.flex = `0 0 ${newA}px`;
        this.items[b].style.flex = `0 0 ${newB}px`;
      };

      const onUp = () => {
        divider.classList.remove("dragging");
        document.body.classList.remove("accordion-drag-active");
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    });
  }

  private toggle(id: PanelId) {
    if (this.openIds.has(id)) {
      this.openIds.delete(id);
      delete this.panelHeights[id];
    } else {
      this.openIds.add(id);
      this.onOpen?.(id);
    }
    this.render();
  }

  private render() {
    for (const id of this.panelOrder) {
      this.container.append(this.items[id]);
    }

    this.container.querySelectorAll(".accordion-divider").forEach((el) => el.remove());

    for (const id of PANEL_IDS) {
      const isOpen = this.openIds.has(id);
      const item = this.items[id];
      item.classList.toggle("open", isOpen);
      item.querySelector<HTMLElement>(".accordion-content")!.hidden = !isOpen;

      const height = this.panelHeights[id];
      item.style.flex = !isOpen ? "0 0 auto" : height !== undefined ? `0 0 ${height}px` : "1 1 0";
    }

    const openInOrder = this.panelOrder.filter((id) => this.openIds.has(id));
    for (let i = 0; i < openInOrder.length - 1; i++) {
      const a = openInOrder[i];
      const b = openInOrder[i + 1];
      const divider = document.createElement("div");
      divider.className = "accordion-divider";
      this.wireDividerDrag(divider, a, b);
      this.items[a].after(divider);
    }
  }
}
