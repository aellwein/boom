import { test } from "node:test";
import assert from "node:assert/strict";

import { convertBrowserTree, type BrowserBookmarkNode } from "../browser-tree.js";
import { summarize } from "../model.js";

// Shape of Chromium's bookmark tree: a root whose children are the three roots.
const browserRoot: BrowserBookmarkNode = {
  title: "",
  children: [
    {
      title: "Bookmarks bar",
      children: [
        { title: "Docs", url: "https://docs.example" },
        {
          title: "Work",
          children: [{ title: "Notes", url: "https://notes.example" }],
        },
      ],
    },
    { title: "Other bookmarks", children: [] },
    { title: "Mobile bookmarks", children: [{ title: "Mail", url: "https://mail.example" }] },
  ],
};

test("keeps the three roots and drops the synthetic root", () => {
  const tree = convertBrowserTree(browserRoot);
  assert.deepEqual(
    tree.map((node) => node.title),
    ["Bookmarks bar", "Other bookmarks", "Mobile bookmarks"],
  );
});

test("a node with a url is a bookmark, one without is a folder", () => {
  const tree = convertBrowserTree(browserRoot);
  const bar = tree[0];
  assert.equal(bar.kind, "folder");
  assert.deepEqual(bar.kind === "folder" ? bar.children[0] : null, {
    kind: "bookmark",
    title: "Docs",
    url: "https://docs.example",
  });
});

test("keeps the order of siblings", () => {
  const tree = convertBrowserTree(browserRoot);
  const bar = tree[0];
  assert.equal(bar.kind, "folder");
  assert.deepEqual(
    bar.kind === "folder" ? bar.children.map((node) => node.title) : [],
    ["Docs", "Work"],
  );
});

test("summary counts folders and bookmarks at every depth", () => {
  const tree = convertBrowserTree(browserRoot);
  // Folders: Bookmarks bar, Work, Other bookmarks, Mobile bookmarks.
  // Bookmarks: Docs, Notes, Mail.
  assert.deepEqual(summarize(tree), { folders: 4, bookmarks: 3 });
});
