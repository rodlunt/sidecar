import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";
import { FileTree } from "./file-tree";
import { GitGraph } from "./git-graph";
import { IssuesPanel } from "./issues-panel";
import { Accordion } from "./accordion";

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

new Accordion(document.getElementById("sidebar-accordion")!, (id) => {
  if (id === "git") gitGraph.refresh();
  if (id === "issues") issuesPanel.refresh();
});

const term = new Terminal({
  cursorBlink: true,
  fontFamily: "'IBM Plex Mono', Menlo, Consolas, monospace",
  fontSize: 14,
  theme: {
    background: "#171a1f",
    foreground: "#e2e4e8",
    cursor: "#4a90d9",
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
