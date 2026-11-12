//! Plans the first-sync overwrite: browser bookmarks are replaced by the
//! repo's. Pure logic, so it is tested without browser APIs.
//!
//! The repo's top-level folders name the browser roots. Their names are
//! fixed (not translated), so they match on every machine.

import type { RepoEntry } from "./protocol.js";

/// Browser IDs of the three roots. Chromium uses these on every profile.
export const ROOT_IDS = { bar: "1", other: "2", mobile: "3" } as const;

/// Top-level repo folder names, mapped to their browser root.
export const ROOT_FOLDERS: Record<string, string> = {
  "bookmarks-bar": ROOT_IDS.bar,
  other: ROOT_IDS.other,
  mobile: ROOT_IDS.mobile,
};

/// Top-level folders that are not one of the roots go here.
const DEFAULT_ROOT = ROOT_IDS.other;

export interface Create {
  kind: "folder" | "bookmark";
  /// Repo path of the entry. Used as the key of the ID map.
  path: string;
  /// Browser ID of the parent, or a repo path whose browser ID the executor
  /// looks up in the ID map.
  parent: { browserId: string } | { repoPath: string };
  index: number;
  title: string;
  url?: string;
}

/// Name of a top-level repo folder: its last path component, without prefix.
function folderName(entry: RepoEntry): string {
  const name = entry.path.split("/").pop() ?? entry.path;
  const match = /^\d+_(.*)$/.exec(name);
  return match ? match[1] : name;
}

function parentPath(path: string): string | null {
  const slash = path.lastIndexOf("/");
  return slash === -1 ? null : path.slice(0, slash);
}

function depth(path: string): number {
  return path.split("/").length;
}

/// Creation steps for every repo entry, parents before children.
///
/// A top-level root folder is not created. Its children go straight into
/// the browser root, and its path maps to that root's ID.
export function planOverwrite(entries: RepoEntry[]): { roots: Record<string, string>; creates: Create[] } {
  const roots: Record<string, string> = {};
  const sorted = [...entries].sort((a, b) => depth(a.path) - depth(b.path) || a.path.localeCompare(b.path));
  const nextIndex = new Map<string, number>();
  const creates: Create[] = [];

  for (const entry of sorted) {
    const parent = parentPath(entry.path);
    const isTopLevel = parent === null;

    if (isTopLevel && entry.kind === "folder" && ROOT_FOLDERS[folderName(entry)] !== undefined) {
      roots[entry.path] = ROOT_FOLDERS[folderName(entry)];
      continue;
    }

    const parentRef =
      parent === null
        ? { browserId: entry.kind === "bookmark" ? ROOT_IDS.bar : DEFAULT_ROOT }
        : parent in roots
          ? { browserId: roots[parent] }
          : { repoPath: parent };
    // Index counts per real parent: a browser ID for roots, a repo path otherwise.
    const slot = "browserId" in parentRef ? `browser:${parentRef.browserId}` : `path:${parentRef.repoPath}`;
    const index = nextIndex.get(slot) ?? 0;
    nextIndex.set(slot, index + 1);

    if (entry.kind === "bookmark") {
      creates.push({
        kind: "bookmark",
        path: entry.path,
        parent: parentRef,
        index,
        title: entry.title,
        url: entry.url ?? "",
      });
    } else {
      creates.push({ kind: "folder", path: entry.path, parent: parentRef, index, title: entry.title });
    }
  }
  return { roots, creates };
}
