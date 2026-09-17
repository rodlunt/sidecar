import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { FileTree } from "./file-tree";
import { GitGraph } from "./git-graph";

const container = document.getElementById("terminal")!;

const fileTree = new FileTree(
  document.getElementById("file-tree")!,
  document.getElementById("root-label")!,
);

const gitGraph = new GitGraph(
  document.getElementById("git-graph-container")!,
  document.getElementById("git-graph-status")!,
);

document.getElementById("open-folder-btn")!.addEventListener("click", async () => {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const picked = await open({ directory: true });
  if (typeof picked === "string") {
    await fileTree.open(picked);
    await gitGraph.open(picked);
  }
});

const tabButtons = document.querySelectorAll<HTMLButtonElement>(".sidebar-tab");
const panels: Record<string, HTMLElement> = {
  files: document.getElementById("file-tree")!,
  git: document.getElementById("git-graph-panel")!,
};

tabButtons.forEach((btn) => {
  btn.addEventListener("click", () => {
    const tab = btn.dataset.tab!;
    tabButtons.forEach((b) => b.classList.toggle("active", b === btn));
    for (const [name, panel] of Object.entries(panels)) {
      panel.hidden = name !== tab;
    }
    if (tab === "git") {
      gitGraph.refresh();
    }
  });
});

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
