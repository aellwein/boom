//! Builds the machine-local ID map (repo path to browser ID) by pairing the
//! browser tree with the repo's entries. Pure logic, so it is tested without
//! browser APIs.
//!
//! A repo write keeps the browser's order, and git does not store empty
//! folders. So a browser folder with no bookmarks anywhere below it has no
//! repo entry, and is skipped here.

import type { BrowserBookmarkNode } from "./browser-tree.js";
import { ROOT_FOLDERS, ROOT_IDS } from "./overwrite-plan.js";
import type { RepoEntry } from "./protocol.js";

export type IdMap = Record<string, string>;
export type PairResult = { ok: true; idMap: IdMap } | { ok: false; reason: string };

function hasBookmark(node: BrowserBookmarkNode): boolean {
  if (node.url !== undefined) {
    return true;
  }
  return (node.children ?? []).some(hasBookmark);
}

/// Browser children that have a repo entry: bookmarks and non-empty folders.
function writable(children: BrowserBookmarkNode[]): BrowserBookmarkNode[] {
  return children.filter((child) => child.url !== undefined || hasBookmark(child));
}

function parentOf(path: string): string | null {
  const slash = path.lastIndexOf("/");
  return slash === -1 ? null : path.slice(0, slash);
}

/// Pairs the browser roots with the repo's top-level folders, then pairs each
/// level below by position. Fails with a reason if the two do not match.
export function buildIdMap(root: BrowserBookmarkNode, entries: RepoEntry[]): PairResult {
  const byParent = new Map<string, RepoEntry[]>();
  for (const entry of entries) {
    const parent = parentOf(entry.path) ?? "";
    const list = byParent.get(parent) ?? [];
    list.push(entry);
    byParent.set(parent, list);
  }
  for (const list of byParent.values()) {
    list.sort((a, b) => (a.path < b.path ? -1 : a.path > b.path ? 1 : 0));
  }

  const idMap: IdMap = {};
  const topLevel = byParent.get("") ?? [];
  const rootNames = Object.entries(ROOT_FOLDERS);
  const browserRoots = (root.children ?? []).filter((child) =>
    Object.values(ROOT_IDS).includes(child.id as (typeof ROOT_IDS)[keyof typeof ROOT_IDS]),
  );

  for (const [name, rootId] of rootNames) {
    const repoRoot = topLevel.find((entry) => entry.kind === "folder" && entry.path.replace(/^\d+_/, "") === name);
    const browserRoot = browserRoots.find((child) => child.id === rootId);
    const browserChildren = browserRoot ? writable(browserRoot.children ?? []) : [];
    if (repoRoot === undefined) {
      if (browserChildren.length > 0) {
        return { ok: false, reason: `the repo has no ${name} folder for ${browserChildren.length} browser item(s)` };
      }
      continue;
    }
    idMap[repoRoot.path] = rootId;
    const pairing = pairLevel(browserChildren, repoRoot.path, byParent, idMap, name);
    if (!pairing.ok) {
      return pairing;
    }
  }

  const known = new Set(rootNames.map(([name]) => name));
  for (const entry of topLevel) {
    if (!known.has(entry.path.replace(/^\d+_/, ""))) {
      return { ok: false, reason: `the repo has an unexpected top-level entry ${entry.path}` };
    }
  }
  return { ok: true, idMap };
}

function pairLevel(
  children: BrowserBookmarkNode[],
  parentPath: string,
  byParent: Map<string, RepoEntry[]>,
  idMap: IdMap,
  label: string,
): PairResult {
  const repoKids = byParent.get(parentPath) ?? [];
  if (repoKids.length !== children.length) {
    return {
      ok: false,
      reason: `${label}: ${children.length} browser item(s) but ${repoKids.length} repo entr(y/ies)`,
    };
  }
  for (let i = 0; i < children.length; i += 1) {
    const child = children[i];
    const entry = repoKids[i];
    const isBookmark = child.url !== undefined;
    if ((entry.kind === "bookmark") !== isBookmark) {
      return { ok: false, reason: `${label}: item ${i + 1} differs in kind from ${entry.path}` };
    }
    idMap[entry.path] = child.id ?? "";
    if (!isBookmark) {
      const nested = pairLevel(writable(child.children ?? []), entry.path, byParent, idMap, entry.path);
      if (!nested.ok) {
        return nested;
      }
    }
  }
  return { ok: true, idMap };
}

/// True if the repo entries include at least one bookmark.
export function repoHasBookmarks(entries: RepoEntry[]): boolean {
  return entries.some((entry) => entry.kind === "bookmark");
}
