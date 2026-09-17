import { invoke } from "@tauri-apps/api/core";
import { layoutCommits } from "./git-graph/layout";
import { renderGraph } from "./git-graph/render";
import type { CommitInput } from "./git-graph/layout";
import type { RenderedCommitInfo } from "./git-graph/render";

interface GraphCommit {
  id: string;
  summary: string;
  author: string;
  date: number;
  parents: string[];
  truncated_parents: string[];
  refs: string[];
  pr_number: number | null;
  pr_title: string | null;
}

interface GraphData {
  commits: GraphCommit[];
  is_shallow: boolean;
}

const LOG_LIMIT = 300;

export class GitGraph {
  private container: HTMLElement;
  private statusLabel: HTMLElement;
  private rootPath: string | null = null;

  constructor(container: HTMLElement, statusLabel: HTMLElement) {
    this.container = container;
    this.statusLabel = statusLabel;
  }

  async open(path: string) {
    this.rootPath = path;
    await this.refresh();
  }

  async refresh() {
    if (!this.rootPath) return;
    this.statusLabel.textContent = "Loading…";
    try {
      const data = await invoke<GraphData>("git_log", { path: this.rootPath, limit: LOG_LIMIT });
      this.render(data);
    } catch (err) {
      console.error("git_log failed", err);
      this.statusLabel.textContent = "Not a git repository";
      this.container.replaceChildren();
    }
  }

  private render(data: GraphData) {
    const commits: CommitInput[] = data.commits.map((c) => ({
      id: c.id,
      parents: c.parents,
      hasTruncatedParent: c.truncated_parents.length > 0,
    }));
    const info: RenderedCommitInfo[] = data.commits.map((c, i) => ({
      index: i,
      summary: c.summary,
      label: c.pr_number !== null && c.pr_title !== null ? `#${c.pr_number} ${c.pr_title}` : c.summary,
      author: c.author,
      date: c.date,
      refs: c.refs,
    }));

    const layout = layoutCommits(commits);
    renderGraph(this.container, layout, info);

    const shallowNote = data.is_shallow ? " (shallow clone)" : "";
    this.statusLabel.textContent = `${data.commits.length} commits${shallowNote}`;
  }
}
