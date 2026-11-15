//! Replaces the browser's bookmarks with the repo's. This is destructive:
//! the setup page asks for confirmation before it calls this.
//!
//! The browser ID of every repo folder and bookmark is kept in extension
//! storage, under `idMap`. It stays on this machine and is never committed.

import { callHost } from "./host.js";
import { ID_MAP_KEY, LAST_HEAD_KEY, SUSPEND_KEY } from "./keys.js";
import { planOverwrite, ROOT_IDS } from "./overwrite-plan.js";
import type { HeadResult, RepoEntry } from "./protocol.js";

export interface OverwriteReport {
  removed: number;
  created: number;
}

export async function overwriteFromRepo(): Promise<OverwriteReport> {
  const head = await callHost<HeadResult>({ action: "repo_head" });
  if (!head.ok || !head.result) {
    throw new Error(head.error ?? "could not read the repo");
  }
  const reply = await callHost<RepoEntry[]>({ action: "repo_entries" });
  if (!reply.ok || !reply.result) {
    throw new Error(reply.error ?? "could not read the repo");
  }
  const entries = reply.result;
  if (!entries.some((entry) => entry.kind === "bookmark")) {
    // A new repo has no bookmarks yet. Removing the browser's bookmarks for
    // an empty repo would lose them, so refuse.
    throw new Error("the repo has no bookmarks yet, so nothing was replaced");
  }

  const { roots, creates } = planOverwrite(entries);

  // Not every profile has every root. Chromium has no "Mobile bookmarks" folder
  // unless it is in use, and getChildren on a missing id fails. Check before
  // anything is removed, so a missing root cannot leave the browser half replaced.
  const [browserRoot] = await chrome.bookmarks.getTree();
  const existingRoots = (browserRoot.children ?? []).filter((node) => (Object.values(ROOT_IDS) as string[]).includes(node.id));
  const existingIds = new Set(existingRoots.map((node) => node.id));
  const missing = creates
    .map((step) => ("browserId" in step.parent ? step.parent.browserId : undefined))
    .filter((id): id is string => id !== undefined && !existingIds.has(id));
  if (missing.length > 0) {
    throw new Error(`the browser has no root folder with id ${[...new Set(missing)].join(", ")}, so nothing was replaced`);
  }

  // Browser writes pause while this runs, so its own edits are not sent back.
  await chrome.storage.local.set({ [SUSPEND_KEY]: true });
  const idMap: Record<string, string> = { ...roots };
  let removed = 0;
  let created = 0;
  let completed = false;
  try {
    for (const root of existingRoots) {
      for (const child of root.children ?? []) {
        await chrome.bookmarks.removeTree(child.id);
        removed += 1;
      }
    }

    for (const step of creates) {
      const parentId = "browserId" in step.parent ? step.parent.browserId : idMap[step.parent.repoPath];
      if (parentId === undefined) {
        throw new Error(`no browser folder for ${"repoPath" in step.parent ? step.parent.repoPath : ""}`);
      }
      const node = await chrome.bookmarks.create({
        parentId,
        title: step.title,
        url: step.kind === "bookmark" ? step.url : undefined,
        index: step.index,
      });
      idMap[step.path] = node.id;
      created += 1;
    }
    completed = true;
  } finally {
    if (completed) {
      // The browser now mirrors the head that was read at the start.
      await chrome.storage.local.set({ [ID_MAP_KEY]: idMap, [LAST_HEAD_KEY]: head.result.head });
    } else {
      // Part of the browser was replaced. Forget the ID map and head, so
      // browser writes stop ("the first sync has not run") instead of sending
      // the half-replaced tree to the repo.
      await chrome.storage.local.remove([ID_MAP_KEY, LAST_HEAD_KEY]);
    }
    await chrome.storage.local.set({ [SUSPEND_KEY]: false });
  }
  return { removed, created };
}
