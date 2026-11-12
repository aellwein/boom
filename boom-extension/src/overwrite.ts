//! Replaces the browser's bookmarks with the repo's. This is destructive:
//! the setup page asks for confirmation before it calls this.
//!
//! The browser ID of every repo folder and bookmark is kept in extension
//! storage, under `idMap`. It stays on this machine and is never committed.

import { callHost } from "./host.js";
import { planOverwrite, ROOT_IDS } from "./overwrite-plan.js";
import type { RepoEntry } from "./protocol.js";

export const ID_MAP_KEY = "idMap";

export interface OverwriteReport {
  removed: number;
  created: number;
}

export async function overwriteFromRepo(): Promise<OverwriteReport> {
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

  let removed = 0;
  for (const rootId of Object.values(ROOT_IDS)) {
    const children = await chrome.bookmarks.getChildren(rootId);
    for (const child of children) {
      await chrome.bookmarks.removeTree(child.id);
      removed += 1;
    }
  }

  const idMap: Record<string, string> = { ...roots };
  let created = 0;
  try {
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
  } finally {
    // Save what was created, even if a later step failed.
    await chrome.storage.local.set({ [ID_MAP_KEY]: idMap });
  }
  return { removed, created };
}
