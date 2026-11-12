import { test } from "node:test";
import assert from "node:assert/strict";

import { applyRepoChanges, type BrowserOps, type IdMap } from "../repo-apply-logic.js";
import type { RepoChange } from "../protocol.js";
import { ROOT_IDS } from "../overwrite-plan.js";

/// Records browser calls and hands out predictable IDs.
function fakeOps() {
  const calls: string[] = [];
  let next = 100;
  const ops: BrowserOps = {
    async create(detail) {
      const id = String(next++);
      calls.push(`create ${id} parent=${detail.parentId} index=${detail.index} title=${detail.title}`);
      return id;
    },
    async update(id, detail) {
      calls.push(`update ${id} title=${detail.title} url=${detail.url}`);
    },
    async remove(id) {
      calls.push(`remove ${id}`);
    },
    async removeTree(id) {
      calls.push(`removeTree ${id}`);
    },
  };
  return { ops, calls };
}

const rootMap = (): IdMap => ({ "0010_bookmarks-bar": ROOT_IDS.bar });

test("creates a folder and a bookmark under the mapped root, with their index", async () => {
  const { ops, calls } = fakeOps();
  const idMap = rootMap();
  const changes: RepoChange[] = [
    { op: "create_folder", path: "0010_bookmarks-bar/0010_Work", parent: "0010_bookmarks-bar", index: 0, title: "Work" },
    { op: "write_bookmark", path: "0010_bookmarks-bar/0010_Work/0010_Docs.toml", parent: "0010_bookmarks-bar/0010_Work", index: 0, title: "Docs", url: "https://docs.example" },
  ];
  await applyRepoChanges(changes, idMap, ops);
  assert.deepEqual(calls, [
    `create 100 parent=${ROOT_IDS.bar} index=0 title=Work`,
    "create 101 parent=100 index=0 title=Docs",
  ]);
  assert.equal(idMap["0010_bookmarks-bar/0010_Work"], "100");
});

test("a folder that is already mapped, such as a root, is not created again", async () => {
  const { ops, calls } = fakeOps();
  await applyRepoChanges(
    [{ op: "create_folder", path: "0010_bookmarks-bar", parent: null, index: 0, title: "bookmarks-bar" }],
    rootMap(),
    ops,
  );
  assert.deepEqual(calls, []);
});

test("a removed bookmark is removed from the browser and the map", async () => {
  const { ops, calls } = fakeOps();
  const idMap: IdMap = { ...rootMap(), "0010_bookmarks-bar/0010_Docs.toml": "7" };
  await applyRepoChanges(
    [{ op: "remove_bookmark", path: "0010_bookmarks-bar/0010_Docs.toml" }],
    idMap,
    ops,
  );
  assert.deepEqual(calls, ["remove 7"]);
  assert.equal(idMap["0010_bookmarks-bar/0010_Docs.toml"], undefined);
});

test("a removed folder removes its subtree and every mapped entry under it", async () => {
  const { ops, calls } = fakeOps();
  const idMap: IdMap = {
    ...rootMap(),
    "0010_bookmarks-bar/0010_Work": "8",
    "0010_bookmarks-bar/0010_Work/0010_Docs.toml": "9",
    "0010_bookmarks-bar/0020_Other": "10",
  };
  await applyRepoChanges([{ op: "remove_folder", path: "0010_bookmarks-bar/0010_Work" }], idMap, ops);
  assert.deepEqual(calls, ["removeTree 8"]);
  assert.equal(idMap["0010_bookmarks-bar/0010_Work/0010_Docs.toml"], undefined);
  assert.equal(idMap["0010_bookmarks-bar/0020_Other"], "10");
});

test("a changed bookmark is updated in place, not created again", async () => {
  const { ops, calls } = fakeOps();
  const idMap: IdMap = { ...rootMap(), "0010_bookmarks-bar/0010_Docs.toml": "7" };
  await applyRepoChanges(
    [{ op: "write_bookmark", path: "0010_bookmarks-bar/0010_Docs.toml", parent: "0010_bookmarks-bar", index: 0, title: "Docs", url: "https://new.example" }],
    idMap,
    ops,
  );
  assert.deepEqual(calls, ["update 7 title=Docs url=https://new.example"]);
});

test("top-level bookmarks go to the bookmarks bar, top-level folders to other", async () => {
  const { ops, calls } = fakeOps();
  await applyRepoChanges(
    [
      { op: "write_bookmark", path: "0010_A.toml", parent: null, index: 0, title: "A", url: "https://a.example" },
      { op: "create_folder", path: "0020_Archive", parent: null, index: 0, title: "Archive" },
    ],
    {},
    ops,
  );
  assert.deepEqual(calls, [
    `create 100 parent=${ROOT_IDS.bar} index=0 title=A`,
    `create 101 parent=${ROOT_IDS.other} index=0 title=Archive`,
  ]);
});

test("a missing parent is an error, not a guess", async () => {
  const { ops } = fakeOps();
  await assert.rejects(
    applyRepoChanges(
      [{ op: "write_bookmark", path: "0010_x/0010_A.toml", parent: "0010_x", index: 0, title: "A", url: "https://a.example" }],
      {},
      ops,
    ),
    /no browser folder for 0010_x/,
  );
});
