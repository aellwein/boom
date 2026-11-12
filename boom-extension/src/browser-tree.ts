//! Converts the browser's bookmark tree into the extension's model.
//!
//! The browser root has no meaning for the repo, so only its children are
//! kept: the bookmarks bar, other bookmarks and mobile bookmarks.

import type { TreeNode } from "./model.js";
import { ROOT_IDS } from "./overwrite-plan.js";

/// The subset of `chrome.bookmarks.BookmarkTreeNode` that conversion needs.
export interface BrowserBookmarkNode {
  id?: string;
  title: string;
  url?: string;
  children?: BrowserBookmarkNode[];
}

/// Converts the browser's synthetic root. Its children become the top-level
/// nodes, so the result holds the three roots as folders.
export function convertBrowserTree(root: BrowserBookmarkNode): TreeNode[] {
  return convertChildren(root);
}

function convertChildren(node: BrowserBookmarkNode): TreeNode[] {
  return (node.children ?? []).map(convertNode);
}

function convertNode(node: BrowserBookmarkNode): TreeNode {
  if (node.url !== undefined) {
    return { kind: "bookmark", title: node.title, url: node.url };
  }
  return { kind: "folder", title: node.title, children: convertChildren(node) };
}

/// The three browser roots, in repo order, with their fixed repo names.
const REPO_ROOTS = [
  { id: ROOT_IDS.bar, name: "bookmarks-bar" },
  { id: ROOT_IDS.other, name: "other" },
  { id: ROOT_IDS.mobile, name: "mobile" },
] as const;

/// The repo's view of the browser: three top-level folders with fixed names.
/// A root missing from the browser becomes an empty folder.
export function repoTreeFromBrowser(root: BrowserBookmarkNode): TreeNode[] {
  const children = root.children ?? [];
  return REPO_ROOTS.map(({ id, name }) => {
    const browserRoot = children.find((child) => child.id === id);
    return {
      kind: "folder" as const,
      title: name,
      children: browserRoot ? convertChildren(browserRoot) : [],
    };
  });
}

