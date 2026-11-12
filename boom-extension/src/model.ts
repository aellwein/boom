//! Bookmark tree as the extension sees it. It mirrors the `boom-bookmarks`
//! model: a folder holds an ordered list of nodes, and a bookmark has a
//! title and a URL. Browser IDs are not part of the model.

export interface BookmarkNode {
  kind: "bookmark";
  title: string;
  url: string;
}

export interface FolderNode {
  kind: "folder";
  title: string;
  children: TreeNode[];
}

export type TreeNode = BookmarkNode | FolderNode;

export interface TreeSummary {
  folders: number;
  bookmarks: number;
}

export function summarize(nodes: TreeNode[]): TreeSummary {
  const summary: TreeSummary = { folders: 0, bookmarks: 0 };
  for (const node of nodes) {
    if (node.kind === "bookmark") {
      summary.bookmarks += 1;
    } else {
      summary.folders += 1;
      const inner = summarize(node.children);
      summary.folders += inner.folders;
      summary.bookmarks += inner.bookmarks;
    }
  }
  return summary;
}
