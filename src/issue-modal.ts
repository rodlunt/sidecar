import { invoke } from "@tauri-apps/api/core";

interface IssueSummaryLike {
  number: number;
  title: string;
}

interface IssueDetail {
  number: number;
  title: string;
  html_url: string;
  body: string | null;
  author: string;
  created_at: number;
  updated_at: number;
  comments: number;
  labels: string[];
}

export async function showIssueModal(path: string, issue: IssueSummaryLike) {
  const backdrop = document.createElement("div");
  backdrop.className = "issue-modal-backdrop";

  const modal = document.createElement("div");
  modal.className = "issue-modal";
  backdrop.append(modal);

  const onKeydown = (e: KeyboardEvent) => {
    if (e.key === "Escape") close();
  };

  function close() {
    backdrop.remove();
    document.removeEventListener("keydown", onKeydown);
  }

  const header = document.createElement("div");
  header.className = "issue-modal-header";

  const titleWrap = document.createElement("div");
  const title = document.createElement("div");
  title.className = "issue-modal-title";
  title.textContent = issue.title;
  const number = document.createElement("div");
  number.className = "issue-modal-number";
  number.textContent = `#${issue.number}`;
  titleWrap.append(title, number);

  const closeBtn = document.createElement("button");
  closeBtn.className = "issue-modal-close";
  closeBtn.textContent = "✕";
  closeBtn.setAttribute("aria-label", "Close");
  closeBtn.addEventListener("click", close);

  header.append(titleWrap, closeBtn);
  modal.append(header);

  const body = document.createElement("div");
  body.className = "issue-modal-body";
  body.textContent = "Loading…";
  modal.append(body);

  backdrop.addEventListener("click", (e) => {
    if (e.target === backdrop) close();
  });
  document.addEventListener("keydown", onKeydown);

  document.body.append(backdrop);

  try {
    const detail = await invoke<IssueDetail>("get_issue_detail", { path, number: issue.number });
    renderDetail(modal, header, body, detail);
  } catch (err) {
    console.error("get_issue_detail failed", err);
    body.textContent = `Could not load this issue: ${String(err)}`;
  }
}

function renderDetail(modal: HTMLElement, header: Element, body: HTMLElement, detail: IssueDetail) {
  const meta = document.createElement("div");
  meta.className = "issue-modal-meta";
  const opened = new Date(detail.created_at * 1000).toLocaleDateString();
  const labelPart = detail.labels.length > 0 ? `, labels: ${detail.labels.join(", ")}` : "";
  meta.textContent = `${detail.author}, opened ${opened}, ${detail.comments} comments${labelPart}`;
  header.after(meta);

  body.textContent = detail.body && detail.body.trim().length > 0 ? detail.body : "No description.";

  const footer = document.createElement("div");
  footer.className = "issue-modal-footer";
  const link = document.createElement("a");
  link.href = detail.html_url;
  link.textContent = "Open on GitHub";
  link.addEventListener("click", (e) => {
    e.preventDefault();
    import("@tauri-apps/plugin-opener").then(({ openUrl }) => openUrl(detail.html_url));
  });
  footer.append(link);
  modal.append(footer);
}
