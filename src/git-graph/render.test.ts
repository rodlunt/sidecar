import { describe, expect, it } from "vitest";
import { Branch, Vertex, type Layout } from "./layout";
import { computeRowLaneWidths, renderGraph, type RenderedCommitInfo } from "./render";

const GRID_X = 14;
const OFFSET_X = 10;
const LABEL_GAP = 8;

/**
 * A quiet-spike-quiet history: rows 0-1 and 5-6 only ever have a single
 * lane in use (the "main" branch), rows 2-4 have three lanes in use
 * because two short-lived side branches ("B" and "C") are alive alongside
 * main for exactly that stretch. This is the shape from issue #11: a brief
 * multi-branch spike in the middle of an otherwise linear history.
 */
function buildSpikeLayout(): Layout {
  const rowCount = 7;
  const vertices = Array.from({ length: rowCount }, (_, i) => new Vertex(i));

  const main = new Branch(0);
  for (let i = 0; i < rowCount; i++) {
    vertices[i].addToBranch(main, 0);
  }
  for (let i = 0; i < rowCount - 1; i++) {
    main.addLine({ x: 0, y: i }, { x: 0, y: i + 1 }, true);
  }

  // Side branches occupy lanes 1 and 2, but only across rows 2-4. They own
  // no vertices of their own (real side branches would, but for this test
  // only the lane occupancy the lines create is under test).
  const sideB = new Branch(1);
  sideB.addLine({ x: 1, y: 2 }, { x: 1, y: 3 }, true);
  sideB.addLine({ x: 1, y: 3 }, { x: 1, y: 4 }, true);

  const sideC = new Branch(2);
  sideC.addLine({ x: 2, y: 2 }, { x: 2, y: 3 }, true);
  sideC.addLine({ x: 2, y: 3 }, { x: 2, y: 4 }, true);

  return { vertices, branches: [main, sideB, sideC], width: 3 };
}

function buildInfo(rowCount: number): RenderedCommitInfo[] {
  return Array.from({ length: rowCount }, (_, i) => ({
    index: i,
    summary: `commit ${i}`,
    label: `commit ${i}`,
    author: "Test Author",
    date: 0,
    refs: [],
  }));
}

describe("computeRowLaneWidths", () => {
  it("reports a wider lane count only for the rows the spike actually spans", () => {
    const layout = buildSpikeLayout();
    const widths = computeRowLaneWidths(layout);

    expect(widths).toEqual([1, 1, 3, 3, 3, 1, 1]);
  });
});

describe("renderGraph label placement", () => {
  it("starts a quiet row's label well before a busy row's label in the same graph", () => {
    const layout = buildSpikeLayout();
    const container = document.createElement("div");
    renderGraph(container, layout, buildInfo(layout.vertices.length));

    const labels = container.querySelectorAll("foreignObject");
    expect(labels.length).toBe(layout.vertices.length);

    const quietRowX = Number(labels[0].getAttribute("x")); // row 0: 1 lane
    const busyRowX = Number(labels[3].getAttribute("x")); // row 3: 3 lanes (spike)
    const quietRowAfterSpikeX = Number(labels[6].getAttribute("x")); // row 6: 1 lane again

    // Criterion 1 & 2: the gap tracks the row's own width, not the global
    // maximum, so the busy row's label sits further right than either quiet
    // row, and the two quiet rows (before and after the spike) match each
    // other rather than both being dragged out to the spike's width.
    expect(busyRowX).toBeGreaterThan(quietRowX);
    expect(quietRowAfterSpikeX).toBe(quietRowX);

    // With the pre-fix behaviour (a single labelStartX derived from
    // layout.width, the global max of 3 lanes), every row's label would sit
    // at the same X as the busiest row. Assert the quiet row is NOT pinned
    // to that global maximum.
    const globalMaxLaneX = 3 * GRID_X + OFFSET_X + LABEL_GAP;
    expect(quietRowX).toBeLessThan(globalMaxLaneX);
  });

  it("keeps a row's label starting after that row's own graph width, never before it", () => {
    const layout = buildSpikeLayout();
    const container = document.createElement("div");
    renderGraph(container, layout, buildInfo(layout.vertices.length));

    const rowWidths = computeRowLaneWidths(layout);
    const labels = container.querySelectorAll("foreignObject");

    for (let row = 0; row < layout.vertices.length; row++) {
      const labelX = Number(labels[row].getAttribute("x"));
      // The rightmost lane in use at this row ends at rowWidths[row] lanes
      // in from the left; the label must start at or after that, with the
      // configured gap, never overlapping the row's own graph lines.
      const rowGraphEndX = rowWidths[row] * GRID_X + OFFSET_X;
      expect(labelX).toBeGreaterThanOrEqual(rowGraphEndX + LABEL_GAP);
    }
  });
});
