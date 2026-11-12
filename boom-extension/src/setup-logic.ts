//! Setup rules, kept free of browser APIs so they can be tested.
//!
//! The setup page collects the form, checks it with `validateSetup`, and
//! turns it into host actions with `buildSetupActions`. The page runs the
//! actions in order and stops at the first failure.

import type { HostAction } from "./protocol.js";

export type RepoMode = "create" | "existing";
export type InitialSync = "merge" | "overwrite";

export interface SetupForm {
  userName: string;
  userEmail: string;
  repoPath: string;
  repoMode: RepoMode;
  remoteUrl: string;
  initialSync: InitialSync;
}

export type SetupCheck =
  | { ok: true; form: SetupForm }
  | { ok: false; errors: string[] };

/// Which repo option the page should preselect. A configured repo folder
/// means the repo exists, so creating one there would fail.
export function initialRepoMode(repoPath: string | null): RepoMode {
  return repoPath === null || repoPath === "" ? "create" : "existing";
}

export function validateSetup(input: SetupForm): SetupCheck {
  const form: SetupForm = {
    userName: input.userName.trim(),
    userEmail: input.userEmail.trim(),
    repoPath: input.repoPath.trim(),
    repoMode: input.repoMode,
    remoteUrl: input.remoteUrl.trim(),
    initialSync: input.initialSync,
  };
  const errors: string[] = [];
  if (form.userName === "") {
    errors.push("Enter your name for commits.");
  }
  if (!/^[^@\s]+@[^@\s]+$/.test(form.userEmail)) {
    errors.push("Enter an email address for commits.");
  }
  if (form.repoPath === "") {
    errors.push("Enter the folder of the bookmark repo.");
  }
  if (form.repoMode !== "create" && form.repoMode !== "existing") {
    errors.push("Choose whether to create a repo or use an existing one.");
  }
  if (form.initialSync !== "merge" && form.initialSync !== "overwrite") {
    errors.push("Choose how the first sync treats existing bookmarks.");
  }
  return errors.length > 0 ? { ok: false, errors } : { ok: true, form };
}

/// Host actions for a valid form, in the order they must run.
///
/// Create mode lets `gitrepo_create` record the repo path. Existing mode
/// sets the path first, because `gitrepo_set_remote` reads it.
export function buildSetupActions(form: SetupForm): HostAction[] {
  const actions: HostAction[] = [
    { action: "config_set", params: { key: "user_name", value: form.userName } },
    { action: "config_set", params: { key: "user_email", value: form.userEmail } },
  ];
  if (form.repoMode === "create") {
    actions.push({
      action: "gitrepo_create",
      params: form.remoteUrl === "" ? { path: form.repoPath } : { path: form.repoPath, remote: form.remoteUrl },
    });
  } else {
    actions.push({ action: "config_set", params: { key: "repo_path", value: form.repoPath } });
    if (form.remoteUrl !== "") {
      actions.push({ action: "gitrepo_set_remote", params: { url: form.remoteUrl } });
    }
  }
  return actions;
}
