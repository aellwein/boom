import { test } from "node:test";
import assert from "node:assert/strict";

import { describeWrite, gateWrite } from "../browser-write.js";
import { repoTreeFromBrowser, type BrowserBookmarkNode } from "../browser-tree.js";

test("gate allows a write when set up, idle and the head is known", () => {
  assert.deepEqual(gateWrite({ hasIdMap: true, lastSeenHead: "abc", suspended: false }), { ok: true, head: "abc" });
});

test("gate refuses before the first sync", () => {
  const gate = gateWrite({ hasIdMap: false, lastSeenHead: "abc", suspended: false });
  assert.equal(gate.ok, false);
});

test("gate refuses while an overwrite runs", () => {
  const gate = gateWrite({ hasIdMap: true, lastSeenHead: "abc", suspended: true });
  assert.equal(gate.ok, false);
});

test("gate refuses when the repo head is unknown", () => {
  assert.equal(gateWrite({ hasIdMap: true, suspended: false }).ok, false);
  assert.equal(gateWrite({ hasIdMap: true, lastSeenHead: "", suspended: false }).ok, false);
});

test("describes a saved and a failed write", () => {
  assert.equal(describeWrite({ at: "t", ok: true }), "Last browser write: saved to the repo");
  assert.equal(
    describeWrite({ at: "t", ok: false, error: "push rejected" }),
    "Last browser write failed: push rejected",
  );
});

test("maps browser roots to fixed repo names by id, in repo order", () => {
  const root: BrowserBookmarkNode = {
    title: "",
    children: [
      { id: "3", title: "Mobile bookmarks", children: [{ title: "Mail", url: "https://mail.example" }] },
      { id: "1", title: "Bookmarks bar", children: [{ title: "Docs", url: "https://docs.example" }] },
      { id: "2", title: "Other bookmarks", children: [] },
    ],
  };
  const tree = repoTreeFromBrowser(root);
  assert.deepEqual(
    tree.map((node) => node.title),
    ["bookmarks-bar", "other", "mobile"],
  );
  assert.deepEqual(tree[0], {
    kind: "folder",
    title: "bookmarks-bar",
    children: [{ kind: "bookmark", title: "Docs", url: "https://docs.example" }],
  });
});

test("a missing browser root becomes an empty folder", () => {
  const tree = repoTreeFromBrowser({ title: "", children: [] });
  assert.deepEqual(
    tree.map((node) => (node.kind === "folder" ? node.children.length : -1)),
    [0, 0, 0],
  );
});
