//! Converts the browser's bookmark tree into the extension's model.
//!
//! The browser root has no meaning for the repo, so only its children are
//! kept: the bookmarks bar, other bookmarks and mobile bookmarks.

import type { TreeNode } from "./model.js";

/// The subset of `chrome.bookmarks.BookmarkTreeNode` that conversion needs.
export interface BrowserBookmarkNode {
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
