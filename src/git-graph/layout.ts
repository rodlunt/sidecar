// Lane-assignment algorithm adapted from mhutchie/vscode-git-graph
// (https://github.com/mhutchie/vscode-git-graph, web/graph.ts, MIT-style licence).
// Trimmed to sidecar's needs: no stash/mute/expand-row/tooltip handling, and no
// "null vertex" sentinel for missing parents: a vertex with an unresolved parent
// is flagged via `hasTruncatedParent` and rendered as an explicit gap, not a line
// that trails off-graph implying unseen-but-real history.

export interface Point {
  readonly x: number;
  readonly y: number;
}

interface Line {
  readonly p1: Point;
  readonly p2: Point;
  readonly lockedFirst: boolean;
}

export interface CommitInput {
  id: string;
  parents: string[]; // parent ids present in this graph
  hasTruncatedParent: boolean;
}

export class Branch {
  constructor(public readonly colour: number) {}
  private lines: Line[] = [];
  private end = 0;

  addLine(p1: Point, p2: Point, lockedFirst: boolean) {
    this.lines.push({ p1, p2, lockedFirst });
  }

  getLines(): ReadonlyArray<Line> {
    return this.lines;
  }

  getEnd() {
    return this.end;
  }

  setEnd(end: number) {
    this.end = end;
  }
}

export class Vertex {
  public readonly id: number;
  public x = 0;
  public hasTruncatedParent = false;
  private children: Vertex[] = [];
  private parents: Vertex[] = [];
  private nextParent = 0;
  private onBranch: Branch | null = null;
  private nextX = 0;
  private connections: { x: number; connectsTo: Vertex | null; onBranch: Branch }[] = [];

  constructor(id: number) {
    this.id = id;
  }

  addChild(v: Vertex) {
    this.children.push(v);
  }

  addParent(v: Vertex) {
    this.parents.push(v);
  }

  getParents(): ReadonlyArray<Vertex> {
    return this.parents;
  }

  isMerge() {
    return this.parents.length > 1;
  }

  getNextParent(): Vertex | null {
    return this.nextParent < this.parents.length ? this.parents[this.nextParent] : null;
  }

  registerParentProcessed() {
    this.nextParent++;
  }

  addToBranch(branch: Branch, x: number) {
    if (this.onBranch === null) {
      this.onBranch = branch;
      this.x = x;
    }
  }

  isNotOnBranch() {
    return this.onBranch === null;
  }

  getBranch() {
    return this.onBranch;
  }

  getColour() {
    return this.onBranch !== null ? this.onBranch.colour : 0;
  }

  getPoint(): Point {
    return { x: this.x, y: this.id };
  }

  getNextPoint(): Point {
    return { x: this.nextX, y: this.id };
  }

  getPointConnectingTo(vertex: Vertex | null, onBranch: Branch): Point | null {
    for (let i = 0; i < this.connections.length; i++) {
      if (this.connections[i].connectsTo === vertex && this.connections[i].onBranch === onBranch) {
        return { x: i, y: this.id };
      }
    }
    return null;
  }

  registerUnavailablePoint(x: number, connectsTo: Vertex | null, onBranch: Branch) {
    if (x === this.nextX) {
      this.nextX = x + 1;
      this.connections[x] = { x, connectsTo, onBranch };
    }
  }
}

export interface Layout {
  vertices: Vertex[];
  branches: Branch[];
  width: number;
}

/**
 * Builds the lane layout for a commit list already in child-before-parent
 * (newest-first) order, matching git2's topological revwalk output.
 */
export function layoutCommits(commits: CommitInput[]): Layout {
  const idIndex = new Map<string, number>();
  commits.forEach((c, i) => idIndex.set(c.id, i));

  const vertices = commits.map((c, i) => {
    const v = new Vertex(i);
    v.hasTruncatedParent = c.hasTruncatedParent;
    return v;
  });

  for (let i = 0; i < commits.length; i++) {
    for (const parentId of commits[i].parents) {
      const parentIndex = idIndex.get(parentId);
      if (parentIndex === undefined) continue; // defensive: shouldn't happen, backend already split these out
      vertices[i].addParent(vertices[parentIndex]);
      vertices[parentIndex].addChild(vertices[i]);
    }
  }

  const branches: Branch[] = [];
  const availableColours: number[] = [];

  const getAvailableColour = (startAt: number) => {
    for (let i = 0; i < availableColours.length; i++) {
      if (startAt > availableColours[i]) return i;
    }
    availableColours.push(0);
    return availableColours.length - 1;
  };

  const determinePath = (startAt: number) => {
    let i = startAt;
    let vertex = vertices[i];
    let parentVertex = vertex.getNextParent();
    let lastPoint = vertex.isNotOnBranch() ? vertex.getNextPoint() : vertex.getPoint();

    if (parentVertex !== null && vertex.isMerge() && !vertex.isNotOnBranch() && !parentVertex.isNotOnBranch()) {
      // Merge into a parent that's already on a branch: connect across to it.
      let found = false;
      const parentBranch = parentVertex.getBranch()!;
      for (i = startAt + 1; i < vertices.length; i++) {
        const curVertex = vertices[i];
        let curPoint = curVertex.getPointConnectingTo(parentVertex, parentBranch);
        if (curPoint !== null) {
          found = true;
        } else {
          curPoint = curVertex.getNextPoint();
        }
        parentBranch.addLine(lastPoint, curPoint, !found && curVertex !== parentVertex ? lastPoint.x < curPoint.x : true);
        curVertex.registerUnavailablePoint(curPoint.x, parentVertex, parentBranch);
        lastPoint = curPoint;
        if (found) {
          vertex.registerParentProcessed();
          break;
        }
      }
    } else {
      const branch = new Branch(getAvailableColour(startAt));
      vertex.addToBranch(branch, lastPoint.x);
      vertex.registerUnavailablePoint(lastPoint.x, vertex, branch);
      for (i = startAt + 1; i < vertices.length; i++) {
        const curVertex = vertices[i];
        const curPoint = parentVertex === curVertex && !parentVertex.isNotOnBranch() ? curVertex.getPoint() : curVertex.getNextPoint();
        branch.addLine(lastPoint, curPoint, lastPoint.x < curPoint.x);
        curVertex.registerUnavailablePoint(curPoint.x, parentVertex, branch);
        lastPoint = curPoint;

        if (parentVertex === curVertex) {
          vertex.registerParentProcessed();
          const parentWasOnBranch = !parentVertex.isNotOnBranch();
          parentVertex.addToBranch(branch, curPoint.x);
          vertex = parentVertex;
          parentVertex = vertex.getNextParent();
          if (parentVertex === null || parentWasOnBranch) break;
        }
      }
      branch.setEnd(i);
      branches.push(branch);
      availableColours[branch.colour] = i;
    }
  };

  let i = 0;
  while (i < vertices.length) {
    if (vertices[i].getNextParent() !== null || vertices[i].isNotOnBranch()) {
      determinePath(i);
    } else {
      i++;
    }
  }

  let width = 0;
  for (const v of vertices) {
    if (v.getNextPoint().x > width) width = v.getNextPoint().x;
  }

  return { vertices, branches, width };
}
