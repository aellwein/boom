//! Applies repo changes to the browser. The browser operations are passed
//! in, so this logic is tested without `chrome.bookmarks`.

import { ROOT_IDS } from "./overwrite-plan.js";
import type { RepoChange } from "./protocol.js";

/// Repo path to browser bookmark ID.
export type IdMap = Record<string, string>;

export interface BrowserOps {
  create(detail: { parentId: string; title: string; url?: string; index: number }): Promise<string>;
  update(id: string, detail: { title: string; url: string }): Promise<void>;
  remove(id: string): Promise<void>;
  removeTree(id: string): Promise<void>;
}

/// Browser parent for an entry without a repo parent: bookmarks go to the
/// bookmarks bar, folders to other bookmarks, as in the overwrite.
function topLevelParent(change: RepoChange): string {
  return change.op === "write_bookmark" ? ROOT_IDS.bar : ROOT_IDS.other;
}

function parentId(change: RepoChange, idMap: IdMap): string {
  if (change.op !== "create_folder" && change.op !== "write_bookmark") {
    throw new Error(`no parent for ${change.op}`);
  }
  if (change.parent === null) {
    return topLevelParent(change);
  }
  const id = idMap[change.parent];
  if (id === undefined) {
    throw new Error(`no browser folder for ${change.parent}`);
  }
  return id;
}

/// Applies the changes in order, updating `idMap` as it goes. The changes
/// come from the planner in a safe order: removals first, then creates with
/// parents before children.
export async function applyRepoChanges(
  changes: RepoChange[],
  idMap: IdMap,
  ops: BrowserOps,
): Promise<void> {
  for (const change of changes) {
    switch (change.op) {
      case "remove_bookmark": {
        const id = idMap[change.path];
        if (id !== undefined) {
          await ops.remove(id);
          delete idMap[change.path];
        }
        break;
      }
      case "remove_folder": {
        const id = idMap[change.path];
        if (id !== undefined) {
          await ops.removeTree(id);
          for (const key of Object.keys(idMap)) {
            if (key === change.path || key.startsWith(`${change.path}/`)) {
              delete idMap[key];
            }
          }
        }
        break;
      }
      case "create_folder": {
        // Already mapped, for example a root folder: nothing to create.
        if (idMap[change.path] !== undefined) {
          break;
        }
        const id = await ops.create({
          parentId: parentId(change, idMap),
          title: change.title,
          index: change.index,
        });
        idMap[change.path] = id;
        break;
      }
      case "write_bookmark": {
        const existing = idMap[change.path];
        if (existing !== undefined) {
          await ops.update(existing, { title: change.title, url: change.url });
        } else {
          const id = await ops.create({
            parentId: parentId(change, idMap),
            title: change.title,
            url: change.url,
            index: change.index,
          });
          idMap[change.path] = id;
        }
        break;
      }
    }
  }
}
