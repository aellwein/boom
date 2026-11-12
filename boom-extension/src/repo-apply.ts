//! Brings the browser up to the repo after a pull. Changes are applied from
//! the last commit the browser saw to the repo's current commit.
//!
//! Browser writes are paused while this runs, so the browser's own edits are
//! not sent back to the repo in the middle of an apply.

import { callHost } from "./host.js";
import { applyRepoChanges, type BrowserOps, type IdMap } from "./repo-apply-logic.js";
import { ID_MAP_KEY, LAST_HEAD_KEY, SUSPEND_KEY } from "./keys.js";
import type { HeadResult, RepoChange } from "./protocol.js";

const chromeOps: BrowserOps = {
  async create(detail) {
    const node = await chrome.bookmarks.create(detail);
    return node.id;
  },
  async update(id, detail) {
    await chrome.bookmarks.update(id, detail);
  },
  async remove(id) {
    await chrome.bookmarks.remove(id);
  },
  async removeTree(id) {
    await chrome.bookmarks.removeTree(id);
  },
};

/// Applies the repo's changes since the last synced head. Does nothing if the
/// browser was never set up, or if the repo has not moved.
export async function applyRepoToBrowser(): Promise<void> {
  const stored = await chrome.storage.local.get([ID_MAP_KEY, LAST_HEAD_KEY]);
  const lastSeen = stored[LAST_HEAD_KEY] as string | undefined;
  const idMapStored = stored[ID_MAP_KEY] as IdMap | undefined;
  if (!lastSeen || !idMapStored) {
    return;
  }

  const head = await callHost<HeadResult>({ action: "repo_head" });
  if (!head.ok || !head.result) {
    throw new Error(head.error ?? "could not read the repo head");
  }
  if (head.result.head === lastSeen) {
    return;
  }

  const changes = await callHost<RepoChange[]>({
    action: "repo_changes_since",
    params: { since: lastSeen },
  });
  if (!changes.ok || !changes.result) {
    throw new Error(changes.error ?? "could not list the repo changes");
  }

  await chrome.storage.local.set({ [SUSPEND_KEY]: true });
  const idMap: IdMap = { ...idMapStored };
  let applied = false;
  try {
    await applyRepoChanges(changes.result, idMap, chromeOps);
    applied = true;
  } finally {
    // Keep the map even after a failure: it lists what was really applied.
    // The head only moves on success, so the next run retries the rest.
    const update: Record<string, unknown> = { [ID_MAP_KEY]: idMap };
    if (applied) {
      update[LAST_HEAD_KEY] = head.result.head;
    }
    await chrome.storage.local.set(update);
    await chrome.storage.local.set({ [SUSPEND_KEY]: false });
  }
}
