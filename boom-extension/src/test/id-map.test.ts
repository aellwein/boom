import { test } from "node:test";
import assert from "node:assert/strict";

import type { BrowserBookmarkNode } from "../browser-tree.js";
import { buildIdMap, repoHasBookmarks } from "../id-map.js";
import type { RepoEntry } from "../protocol.js";

const browser: BrowserBookmarkNode = {
  id: "0",
  title: "",
  children: [
    {
      id: "1",
      title: "Bookmarks bar",
      children: [
        { id: "10", title: "Docs", url: "https://docs.example" },
        { id: "11", title: "Work", children: [{ id: "12", title: "Notes", url: "https://notes.example" }] },
      ],
    },
    { id: "2", title: "Other bookmarks", children: [{ id: "20", title: "Empty folder", children: [] }] },
    { id: "3", title: "Mobile bookmarks", children: [] },
  ],
};

const repo: RepoEntry[] = [
  { path: "0010_bookmarks-bar", kind: "folder", title: "bookmarks-bar", url: null },
  { path: "0010_bookmarks-bar/0010_Docs.toml", kind: "bookmark", title: "Docs", url: "https://docs.example" },
  { path: "0010_bookmarks-bar/0020_Work", kind: "folder", title: "Work", url: null },
  { path: "0010_bookmarks-bar/0020_Work/0010_Notes.toml", kind: "bookmark", title: "Notes", url: "https://notes.example" },
  { path: "0020_other", kind: "folder", title: "other", url: null },
];

test("pairs the browser tree with the repo entries by position", () => {
  const result = buildIdMap(browser, repo);
  assert.equal(result.ok, true);
  if (result.ok) {
    assert.deepEqual(result.idMap, {
      "0010_bookmarks-bar": "1",
      "0010_bookmarks-bar/0010_Docs.toml": "10",
      "0010_bookmarks-bar/0020_Work": "11",
      "0010_bookmarks-bar/0020_Work/0010_Notes.toml": "12",
      "0020_other": "2",
    });
  }
});

test("empty browser folders have no repo entry and are skipped", () => {
  // "Empty folder" under Other bookmarks is not in the repo, and that is fine.
  const result = buildIdMap(browser, repo);
  assert.equal(result.ok, true);
  if (result.ok) {
    assert.equal(Object.values(result.idMap).includes("20"), false);
  }
});

test("a count mismatch fails with a reason and maps nothing", () => {
  const extra: RepoEntry[] = [...repo, { path: "0010_bookmarks-bar/0030_Extra.toml", kind: "bookmark", title: "X", url: "https://x.example" }];
  const result = buildIdMap(browser, extra);
  assert.equal(result.ok, false);
  if (!result.ok) {
    assert.match(result.reason, /bookmarks-bar/);
  }
});

test("a kind mismatch fails", () => {
  const swapped: RepoEntry[] = repo.map((entry) =>
    entry.path === "0010_bookmarks-bar/0010_Docs.toml" ? { ...entry, kind: "folder" as const } : entry,
  );
  assert.equal(buildIdMap(browser, swapped).ok, false);
});

test("a missing root folder is fine when the browser root is empty", () => {
  const onlyBar: RepoEntry[] = repo.filter((entry) => !entry.path.startsWith("0020_other"));
  const bare: BrowserBookmarkNode = { id: "0", title: "", children: [browser.children![0]] };
  assert.equal(buildIdMap(bare, onlyBar).ok, true);
});

test("detects whether the repo has any bookmark", () => {
  assert.equal(repoHasBookmarks(repo), true);
  assert.equal(repoHasBookmarks([{ path: "0010_other", kind: "folder", title: "other", url: null }]), false);
});
