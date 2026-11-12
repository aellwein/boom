//! Setup page. Writes the host config and repo through the host, then
//! remembers the first-sync choice in extension storage.

import { callHost } from "./host.js";
import { schedulePull } from "./schedule.js";
import type { HostConfig, RepoStatus } from "./protocol.js";
import {
  buildSetupActions,
  initialRepoMode,
  validateSetup,
  type InitialSync,
  type RepoMode,
  type SetupForm,
} from "./setup-logic.js";

const form = document.getElementById("setup-form") as HTMLFormElement;
const hostStatus = document.getElementById("host-status") as HTMLParagraphElement;
const message = document.getElementById("message") as HTMLParagraphElement;

function readForm(): SetupForm {
  const data = new FormData(form);
  return {
    userName: String(data.get("userName") ?? ""),
    userEmail: String(data.get("userEmail") ?? ""),
    repoPath: String(data.get("repoPath") ?? ""),
    repoMode: String(data.get("repoMode") ?? "") as RepoMode,
    remoteUrl: String(data.get("remoteUrl") ?? ""),
    initialSync: String(data.get("initialSync") ?? "") as InitialSync,
  };
}

function setField(name: string, value: string | null): void {
  const input = form.elements.namedItem(name) as HTMLInputElement | null;
  if (input && value !== null) {
    input.value = value;
  }
}

function selectRadio(name: string, value: string): void {
  const radio = form.querySelector<HTMLInputElement>(`input[name="${name}"][value="${value}"]`);
  if (radio) {
    radio.checked = true;
  }
}

async function load(): Promise<void> {
  try {
    const reply = await callHost<HostConfig>({ action: "config_show" });
    if (!reply.ok || !reply.result) {
      throw new Error(reply.error ?? "no config returned");
    }
    setField("userName", reply.result.user_name);
    setField("userEmail", reply.result.user_email);
    setField("repoPath", reply.result.repo_path);
    hostStatus.textContent = "Connected to the boom host.";
  } catch (err) {
    hostStatus.textContent = `Host not reachable: ${String(err)}`;
    return;
  }

  const status = await callHost<RepoStatus>({ action: "gitrepo_status" });
  if (status.ok && status.result) {
    setField("remoteUrl", status.result.origin_url);
    selectRadio("repoMode", initialRepoMode(status.result.repo_path));
  } else {
    message.textContent = `Could not read the repo: ${status.error ?? "unknown error"}`;
  }

  const stored = await chrome.storage.local.get("initialSync");
  if (stored.initialSync === "merge" || stored.initialSync === "overwrite") {
    selectRadio("initialSync", stored.initialSync);
  }
}

async function save(event: SubmitEvent): Promise<void> {
  event.preventDefault();
  const check = validateSetup(readForm());
  if (!check.ok) {
    message.textContent = check.errors.join(" ");
    return;
  }

  const actions = buildSetupActions(check.form);
  for (const action of actions) {
    const reply = await callHost(action);
    if (!reply.ok) {
      message.textContent = `Setup stopped: ${reply.error ?? "unknown error"}`;
      return;
    }
  }
  await chrome.storage.local.set({ initialSync: check.form.initialSync });
  await schedulePull();
  message.textContent = "Setup saved.";
}

form.addEventListener("submit", (event) => {
  void save(event);
});
void load();
