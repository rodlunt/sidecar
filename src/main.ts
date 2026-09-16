import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import "@xterm/xterm/css/xterm.css";

const container = document.getElementById("terminal")!;

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
