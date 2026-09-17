import type { Layout, Point } from "./layout";

const SVG_NS = "http://www.w3.org/2000/svg";

const GRID_X = 14;
const GRID_Y = 24;
const OFFSET_X = 10;
const OFFSET_Y = 12;
const GAP_STUB_Y = 10;
const LABEL_GAP = 8;
const LABEL_WIDTH = 200;

// Colour is secondary: branch lane (x position) and node shape (merge vs.
// regular commit) are what actually carry the meaning, so this palette only
// needs to be legible, not uniquely decodable on its own.
const COLOURS = ["#4e9a06", "#3465a4", "#c4a000", "#ce5c00", "#75507b", "#a40000", "#06989a"];

function px(p: Point) {
  return { x: p.x * GRID_X + OFFSET_X, y: p.y * GRID_Y + OFFSET_Y };
}

export interface RenderedCommitInfo {
  index: number;
  summary: string;
  /** Text shown inline next to the node: the PR title for a recognised
   * merge commit, the commit summary otherwise. */
  label: string;
  author: string;
  date: number;
  refs: string[];
}

/**
 * Draws the graph as SVG into `container`, replacing any previous content.
 * `info` supplies per-commit metadata (for the native tooltip) keyed by row index.
 */
export function renderGraph(container: HTMLElement, layout: Layout, info: RenderedCommitInfo[]): void {
  const height = layout.vertices.length * GRID_Y + OFFSET_Y;
  const labelStartX = layout.width * GRID_X + OFFSET_X + LABEL_GAP;
  const width = labelStartX + LABEL_WIDTH;

  const svg = document.createElementNS(SVG_NS, "svg");
  svg.setAttribute("width", String(width));
  svg.setAttribute("height", String(height));
  svg.setAttribute("class", "git-graph-svg");

  for (const branch of layout.branches) {
    const colour = COLOURS[branch.colour % COLOURS.length];
    for (const line of branch.getLines()) {
      const p1 = px(line.p1);
      const p2 = px(line.p2);
      const path = document.createElementNS(SVG_NS, "path");
      let d: string;
      if (p1.x === p2.x) {
        d = `M${p1.x},${p1.y} L${p2.x},${p2.y}`;
      } else {
        const bend = GRID_Y * 0.6;
        d = `M${p1.x},${p1.y} C${p1.x},${p1.y + bend} ${p2.x},${p2.y - bend} ${p2.x},${p2.y}`;
      }
      path.setAttribute("d", d);
      path.setAttribute("class", "git-graph-line");
      path.setAttribute("stroke", colour);
      svg.appendChild(path);
    }
  }

  for (const vertex of layout.vertices) {
    const point = px(vertex.getPoint());
    const colour = COLOURS[vertex.getColour() % COLOURS.length];
    const meta = info[vertex.id];

    const node = document.createElementNS(SVG_NS, vertex.isMerge() ? "rect" : "circle");
    if (vertex.isMerge()) {
      const size = 6;
      node.setAttribute("x", String(point.x - size / 2));
      node.setAttribute("y", String(point.y - size / 2));
      node.setAttribute("width", String(size));
      node.setAttribute("height", String(size));
    } else {
      node.setAttribute("cx", String(point.x));
      node.setAttribute("cy", String(point.y));
      node.setAttribute("r", "4");
    }
    node.setAttribute("fill", colour);
    node.setAttribute("class", "git-graph-vertex");

    if (meta) {
      const title = document.createElementNS(SVG_NS, "title");
      const when = new Date(meta.date * 1000).toLocaleString();
      const refPart = meta.refs.length > 0 ? ` [${meta.refs.join(", ")}]` : "";
      title.textContent = `${meta.summary}${refPart}\n${meta.author}, ${when}`;
      node.appendChild(title);
    }
    svg.appendChild(node);

    if (meta) {
      const foreignObject = document.createElementNS(SVG_NS, "foreignObject");
      foreignObject.setAttribute("x", String(labelStartX));
      foreignObject.setAttribute("y", String(point.y - GRID_Y / 2));
      foreignObject.setAttribute("width", String(LABEL_WIDTH));
      foreignObject.setAttribute("height", String(GRID_Y));

      const label = document.createElement("div");
      label.className = "git-graph-label";
      label.textContent = meta.label;
      foreignObject.appendChild(label);
      svg.appendChild(foreignObject);
    }

    if (vertex.hasTruncatedParent) {
      // History is known to continue but isn't in the fetched set (shallow
      // clone boundary), draw a short dashed stub with an open marker
      // rather than a line that could be misread as connecting to a real commit.
      const stub = document.createElementNS(SVG_NS, "line");
      stub.setAttribute("x1", String(point.x));
      stub.setAttribute("y1", String(point.y));
      stub.setAttribute("x2", String(point.x));
      stub.setAttribute("y2", String(point.y + GAP_STUB_Y));
      stub.setAttribute("stroke", colour);
      stub.setAttribute("class", "git-graph-gap-stub");
      svg.appendChild(stub);

      const marker = document.createElementNS(SVG_NS, "circle");
      marker.setAttribute("cx", String(point.x));
      marker.setAttribute("cy", String(point.y + GAP_STUB_Y));
      marker.setAttribute("r", "2.5");
      marker.setAttribute("class", "git-graph-gap-marker");
      marker.setAttribute("stroke", colour);
      const title = document.createElementNS(SVG_NS, "title");
      title.textContent = "History truncated here (shallow clone or unfetched ancestor)";
      marker.appendChild(title);
      svg.appendChild(marker);
    }
  }

  container.replaceChildren(svg);
}
