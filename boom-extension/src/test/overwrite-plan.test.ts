import { test } from "node:test";
import assert from "node:assert/strict";

import { planOverwrite, ROOT_IDS } from "../overwrite-plan.js";
import type { RepoEntry } from "../protocol.js";

const folder = (path: string, title: string): RepoEntry => ({ path, kind: "folder", title, url: null });
const bookmark = (path: string, title: string, url: string): RepoEntry => ({ path, kind: "bookmark", title, url });

test("top-level root folders are not created; their children go into the root", () => {
  const entries = [
    folder("0010_bookmarks-bar", "bookmarks-bar"),
    bookmark("0010_bookmarks-bar/0010_Docs.toml", "Docs", "https://docs.example"),
  ];
  const { roots, creates } = planOverwrite(entries);
  assert.deepEqual(roots, { "0010_bookmarks-bar": ROOT_IDS.bar });
  assert.equal(creates.length, 1);
  assert.deepEqual(creates[0].parent, { browserId: ROOT_IDS.bar });
  assert.equal(creates[0].index, 0);
});

test("a top-level bookmark goes to the bookmarks bar", () => {
  const { creates } = planOverwrite([bookmark("0010_A.toml", "A", "https://a.example")]);
  assert.deepEqual(creates[0].parent, { browserId: ROOT_IDS.bar });
});

test("an unknown top-level folder goes to other bookmarks", () => {
  const { creates } = planOverwrite([folder("0010_Archive", "Archive")]);
  assert.equal(creates[0].kind, "folder");
  assert.deepEqual(creates[0].parent, { browserId: ROOT_IDS.other });
});

test("nested items refer to their parent by repo path, parents first", () => {
  const entries = [
    bookmark("0010_other/0010_Work/0010_Notes.toml", "Notes", "https://n.example"),
    folder("0010_other", "other"),
    folder("0010_other/0010_Work", "Work"),
  ];
  const { creates } = planOverwrite(entries);
  const paths = creates.map((c) => c.path);
  assert.ok(paths.indexOf("0010_other/0010_Work") < paths.indexOf("0010_other/0010_Work/0010_Notes.toml"));
  const work = creates.find((c) => c.path === "0010_other/0010_Work");
  const notes = creates.find((c) => c.path.endsWith("Notes.toml"));
  assert.deepEqual(work?.parent, { browserId: ROOT_IDS.other });
  assert.deepEqual(notes?.parent, { repoPath: "0010_other/0010_Work" });
});

test("indexes count per real parent, in repo order", () => {
  const entries = [
    bookmark("0010_bookmarks-bar/0010_A.toml", "A", "https://a.example"),
    bookmark("0010_bookmarks-bar/0020_B.toml", "B", "https://b.example"),
    folder("0010_bookmarks-bar", "bookmarks-bar"),
  ];
  const { creates } = planOverwrite(entries);
  assert.deepEqual(creates.map((c) => c.index), [0, 1]);
});

test("siblings under one browser root are numbered in order, across kinds", () => {
  const entries = [
    folder("0010_other", "other"),
    folder("0020_Archive", "Archive"),
    bookmark("0010_other/0010_X.toml", "X", "https://x.example"),
  ];
  const { creates } = planOverwrite(entries);
  const archive = creates.find((c) => c.title === "Archive");
  const x = creates.find((c) => c.title === "X");
  assert.deepEqual(archive?.parent, { browserId: ROOT_IDS.other });
  assert.deepEqual(x?.parent, { browserId: ROOT_IDS.other });
  assert.equal(archive?.index, 0);
  assert.equal(x?.index, 1);
});
