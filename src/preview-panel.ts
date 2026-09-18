import { invoke } from "@tauri-apps/api/core";

type FileContent = { kind: "text"; content: string } | { kind: "binary" };

// A single panel docked above the terminal that shows the contents of
// whichever file was last clicked in the tree. It never stacks: opening a
// new file replaces whatever is currently shown, and a request token
// guards against a slow read for an earlier click overwriting a faster
// one for a later click.
export class PreviewPanel {
  private panel: HTMLElement;
  private pathLabel: HTMLElement;
  private body: HTMLElement;
  private closeBtn: HTMLButtonElement;
  private onToggle: () => void;
  private requestToken = 0;

  constructor(
    panel: HTMLElement,
    pathLabel: HTMLElement,
    body: HTMLElement,
    closeBtn: HTMLButtonElement,
    onToggle: () => void = () => {},
  ) {
    this.panel = panel;
    this.pathLabel = pathLabel;
    this.body = body;
    this.closeBtn = closeBtn;
    this.onToggle = onToggle;
    this.closeBtn.addEventListener("click", () => this.close());
  }

  async open(path: string) {
    const token = ++this.requestToken;
    const wasHidden = this.panel.hidden;
    this.panel.hidden = false;
    this.pathLabel.textContent = path;
    this.pathLabel.title = path;
    this.body.replaceChildren();
    // Only the terminal pane's height actually changes when the panel goes
    // from hidden to shown; swapping content while it's already open
    // doesn't move anything the terminal cares about.
    if (wasHidden) this.onToggle();

    try {
      const result = await invoke<FileContent>("read_file", { path });
      if (token !== this.requestToken) return; // superseded by a later click
      this.renderContent(result);
    } catch (err) {
      if (token !== this.requestToken) return;
      this.renderError(String(err));
    }
  }

  close() {
    if (this.panel.hidden) return;
    this.requestToken++;
    this.panel.hidden = true;
    this.body.replaceChildren();
    this.pathLabel.textContent = "";
    this.pathLabel.title = "";
    this.onToggle();
  }

  private renderContent(result: FileContent) {
    this.body.replaceChildren();
    if (result.kind === "binary") {
      const message = document.createElement("div");
      message.className = "preview-message";
      message.textContent = "Binary file, not previewed.";
      this.body.append(message);
      return;
    }

    const pre = document.createElement("pre");
    pre.className = "preview-text";
    pre.textContent = result.content;
    this.body.append(pre);
  }

  private renderError(detail: string) {
    const message = document.createElement("div");
    message.className = "preview-message preview-message-error";
    message.textContent = `Couldn't read file: ${detail}`;
    this.body.append(message);
  }
}
