import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

interface FileEntry {
  name: string;
  path: string;
  is_dir: boolean;
}

interface NodeState {
  entry: FileEntry;
  expanded: boolean;
  children: NodeState[] | null;
}

export class FileTree {
  private container: HTMLElement;
  private rootLabel: HTMLElement;
  private onFileClick: (path: string) => void;
  private rootPath: string | null = null;
  private roots: NodeState[] = [];

  constructor(
    container: HTMLElement,
    rootLabel: HTMLElement,
    onFileClick: (path: string) => void,
  ) {
    this.container = container;
    this.rootLabel = rootLabel;
    this.onFileClick = onFileClick;
    listen("fs-changed", () => this.refresh());
  }

  async open(path: string) {
    this.rootPath = path;
    this.rootLabel.textContent = path.split("/").pop() || path;
    this.rootLabel.title = path;
    this.roots = await this.loadChildren(path);
    this.render();
    await invoke("watch_root", { path }).catch((err) => {
      console.error("watch_root failed", err);
    });
  }

  private async loadChildren(path: string): Promise<NodeState[]> {
    const entries = await invoke<FileEntry[]>("read_dir", { path });
    return entries.map((entry) => ({ entry, expanded: false, children: null }));
  }

  private async refresh() {
    if (!this.rootPath) return;
    await this.refreshNodes(this.roots);
    this.render();
  }

  private async refreshNodes(nodes: NodeState[]) {
    for (const node of nodes) {
      if (node.entry.is_dir && node.expanded) {
        node.children = await this.loadChildren(node.entry.path);
        await this.refreshNodes(node.children);
      }
    }
  }

  private render() {
    this.container.replaceChildren();
    this.container.append(...this.roots.map((n) => this.renderNode(n)));
  }

  private renderNode(node: NodeState): HTMLElement {
    const wrapper = document.createElement("div");
    wrapper.className = "tree-node";

    const row = document.createElement("div");
    row.className = "tree-row";

    const icon = document.createElement("span");
    icon.className = "tree-icon";
    icon.textContent = node.entry.is_dir ? (node.expanded ? "▾" : "▸") : "";

    const name = document.createElement("span");
    name.className = "tree-name";
    name.textContent = node.entry.name;

    row.append(icon, name);
    wrapper.append(row);

    if (node.entry.is_dir) {
      row.addEventListener("click", () => this.toggle(node, wrapper));
      if (node.expanded && node.children) {
        const childContainer = document.createElement("div");
        childContainer.className = "tree-children";
        childContainer.append(...node.children.map((c) => this.renderNode(c)));
        wrapper.append(childContainer);
      }
    } else {
      row.addEventListener("click", () => this.onFileClick(node.entry.path));
    }

    return wrapper;
  }

  private async toggle(node: NodeState, wrapper: HTMLElement) {
    node.expanded = !node.expanded;
    if (node.expanded && node.children === null) {
      node.children = await this.loadChildren(node.entry.path);
    }
    const rendered = this.renderNode(node);
    wrapper.replaceWith(rendered);
  }
}
