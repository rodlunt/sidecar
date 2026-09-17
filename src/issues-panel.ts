import { invoke } from "@tauri-apps/api/core";
import { showIssueModal } from "./issue-modal";

type RelativeAge = "older_than_usual" | "newer_than_usual" | "typical";

interface IssueSummary {
  number: number;
  title: string;
  html_url: string;
  age_days: number;
  days_since_update: number;
  comments: number;
  relative_to_median: RelativeAge;
}

interface IssuesSummary {
  issues: IssueSummary[];
  median_age_days: number;
}

interface DeviceLoginInfo {
  user_code: string;
  verification_uri: string;
  expires_in: number;
}

export class IssuesPanel {
  private container: HTMLElement;
  private rootPath: string | null = null;

  constructor(container: HTMLElement) {
    this.container = container;
  }

  async open(path: string) {
    this.rootPath = path;
    await this.refresh();
  }

  async refresh() {
    if (!this.rootPath) return;

    let login: string | null;
    try {
      login = await invoke<string | null>("github_auth_status");
    } catch (err) {
      console.error("github_auth_status failed", err);
      login = null;
    }

    if (login === null) {
      this.renderLoginPrompt();
      return;
    }

    this.renderLoading(login);
    try {
      const summary = await invoke<IssuesSummary>("list_issues", { path: this.rootPath });
      this.renderIssues(login, summary);
    } catch (err) {
      console.error("list_issues failed", err);
      this.renderError(String(err));
    }
  }

  private renderLoginPrompt() {
    this.container.replaceChildren();

    const button = document.createElement("button");
    button.className = "issues-signin-btn";
    button.textContent = "Sign in with GitHub";
    button.addEventListener("click", () => this.startDeviceLogin());

    this.container.append(button);
  }

  private async startDeviceLogin() {
    this.container.replaceChildren();
    const status = document.createElement("div");
    status.className = "issues-status";
    status.textContent = "Starting sign-in…";
    this.container.append(status);

    let info: DeviceLoginInfo;
    try {
      info = await invoke<DeviceLoginInfo>("github_device_login_start");
    } catch (err) {
      console.error("github_device_login_start failed", err);
      status.textContent = "Could not start GitHub sign-in.";
      return;
    }

    const code = document.createElement("div");
    code.className = "issues-device-code";
    code.textContent = info.user_code;

    const link = document.createElement("a");
    link.className = "issues-device-link";
    link.href = info.verification_uri;
    link.textContent = "Open " + info.verification_uri;
    link.addEventListener("click", (e) => {
      e.preventDefault();
      import("@tauri-apps/plugin-opener").then(({ openUrl }) => openUrl(info.verification_uri));
    });

    status.textContent = "Enter this code to sign in:";
    this.container.append(code, link);

    try {
      await invoke<string>("github_device_login_poll");
      status.textContent = "Signed in. Loading…";
      await this.refresh();
    } catch (err) {
      console.error("github_device_login_poll failed", err);
      status.textContent = "Sign-in failed or expired. Try again.";
    }
  }

  private renderLoading(login: string) {
    this.container.replaceChildren();
    const status = document.createElement("div");
    status.className = "issues-status";
    status.textContent = `Loading issues as ${login}…`;
    this.container.append(status);
  }

  private renderError(message: string) {
    this.container.replaceChildren();
    const status = document.createElement("div");
    status.className = "issues-status";
    status.textContent = `Could not load issues: ${message}`;
    this.container.append(status);
  }

  private renderIssues(login: string, summary: IssuesSummary) {
    this.container.replaceChildren();

    const header = document.createElement("div");
    header.className = "issues-header";
    header.append(this.buildHeaderText(summary));

    const signOut = document.createElement("button");
    signOut.className = "issues-signout-btn";
    signOut.textContent = "Sign out";
    signOut.title = `Signed in as ${login}`;
    signOut.addEventListener("click", async () => {
      await invoke("github_logout").catch((err) => console.error("github_logout failed", err));
      await this.refresh();
    });

    const headerRow = document.createElement("div");
    headerRow.className = "issues-header-row";
    headerRow.append(header, signOut);
    this.container.append(headerRow);

    const list = document.createElement("div");
    list.className = "issues-list";
    for (const issue of summary.issues) {
      list.append(this.renderIssueRow(issue));
    }
    this.container.append(list);
  }

  private buildHeaderText(summary: IssuesSummary): Text {
    if (summary.issues.length === 0) {
      return document.createTextNode("No open issues");
    }
    const oldest = Math.max(...summary.issues.map((i) => i.age_days));
    return document.createTextNode(
      `${summary.issues.length} open, oldest is ${oldest}d, median ${summary.median_age_days}d`,
    );
  }

  private renderIssueRow(issue: IssueSummary): HTMLElement {
    const row = document.createElement("button");
    row.className = "issues-row";
    row.addEventListener("click", () => {
      if (this.rootPath) showIssueModal(this.rootPath, issue);
    });

    const number = document.createElement("span");
    number.className = "issues-row-number";
    number.textContent = `#${issue.number}`;

    const title = document.createElement("span");
    title.className = "issues-row-title";
    title.textContent = issue.title;

    const meta = document.createElement("span");
    meta.className = "issues-row-meta";
    meta.textContent = this.buildMetaText(issue);
    if (issue.relative_to_median === "older_than_usual") {
      meta.classList.add("issues-row-meta-older");
    } else if (issue.relative_to_median === "newer_than_usual") {
      meta.classList.add("issues-row-meta-newer");
    }

    row.append(number, title, meta);
    return row;
  }

  private buildMetaText(issue: IssueSummary): string {
    const parts = [`${issue.age_days}d old`];
    if (issue.relative_to_median === "older_than_usual") parts.push("older than usual");
    if (issue.relative_to_median === "newer_than_usual") parts.push("newer than usual");
    if (issue.days_since_update > 0) parts.push(`no activity in ${issue.days_since_update}d`);
    return parts.join(", ");
  }
}
