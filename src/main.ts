import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { FileTree } from "./file-tree";
import { GitGraph } from "./git-graph";
import { IssuesPanel } from "./issues-panel";

const container = document.getElementById("terminal")!;

const fileTree = new FileTree(
  document.getElementById("file-tree")!,
  document.getElementById("root-label")!,
);

const gitGraph = new GitGraph(
  document.getElementById("git-graph-container")!,
  document.getElementById("git-graph-status")!,
);

const issuesPanel = new IssuesPanel(document.getElementById("issues-panel")!);

document.getElementById("open-folder-btn")!.addEventListener("click", async () => {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true });
  if (typeof picked === "string") {
    await fileTree.open(picked);
    await gitGraph.open(picked);
    await issuesPanel.open(picked);
  }
});

const PANEL_IDS = ["files", "git", "issues"] as const;
type PanelId = (typeof PANEL_IDS)[number];

const accordion = document.getElementById("sidebar-accordion")!;
const accordionItems: Record<PanelId, HTMLElement> = {
  files: accordion.querySelector('[data-panel="files"]')!,
  git: accordion.querySelector('[data-panel="git"]')!,
  issues: accordion.querySelector('[data-panel="issues"]')!,
};

// Panels open independently (any subset can be visible together) and are
// ordered by when they were opened: newly opened panels append to the
// bottom of the open stack, closed panels fall back to their default order.
let openOrder: PanelId[] = ["files"];

function renderAccordion() {
  const closedIds = PANEL_IDS.filter((id) => !openOrder.includes(id));
  for (const id of [...openOrder, ...closedIds]) {
    accordion.append(accordionItems[id]);
  }
  for (const id of PANEL_IDS) {
    const isOpen = openOrder.includes(id);
    const item = accordionItems[id];
    item.classList.toggle("open", isOpen);
    item.querySelector<HTMLElement>(".accordion-content")!.hidden = !isOpen;
  }
}

function togglePanel(id: PanelId) {
  if (openOrder.includes(id)) {
    openOrder = openOrder.filter((p) => p !== id);
  } else {
    openOrder = [...openOrder, id];
    if (id === "git") gitGraph.refresh();
    if (id === "issues") issuesPanel.refresh();
  }
  renderAccordion();
}

for (const id of PANEL_IDS) {
  accordionItems[id]
    .querySelector(".accordion-header")!
    .addEventListener("click", () => togglePanel(id));
}

renderAccordion();

const term = new Terminal({
  cursorBlink: true,
  fontFamily: "Menlo, Consolas, monospace",
  fontSize: 14,
  theme: {
    background: "#1e1e1e",
    foreground: "#e0e0e0",
  },
});

const fitAddon = new FitAddon();
term.loadAddon(fitAddon);
term.open(container);
fitAddon.fit();

async function spawnShell() {
  await invoke("pty_spawn", { cols: term.cols, rows: term.rows });
}

await listen<string>("pty-output", (event) => {
  term.write(event.payload);
});

await listen("pty-exit", () => {
  term.write("\r\n[process exited]\r\n");
});

term.onData((data) => {
  invoke("pty_write", { data }).catch((err) => {
    console.error("pty_write failed", err);
  });
});

let resizeTimer: ReturnType<typeof setTimeout> | undefined;
window.addEventListener("resize", () => {
  clearTimeout(resizeTimer);
  resizeTimer = setTimeout(() => {
    fitAddon.fit();
    invoke("pty_resize", { cols: term.cols, rows: term.rows }).catch((err) => {
      console.error("pty_resize failed", err);
    });
  }, 100);
});

await spawnShell();
term.focus();
