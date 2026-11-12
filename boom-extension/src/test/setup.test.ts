import { test } from "node:test";
import assert from "node:assert/strict";

import {
  buildSetupActions,
  initialRepoMode,
  validateSetup,
  type SetupForm,
} from "../setup-logic.js";

const valid: SetupForm = {
  userName: "Alex Ellwein",
  userEmail: "alex@example.com",
  repoPath: "/Users/alex/bookmarks",
  repoMode: "create",
  remoteUrl: "",
  initialSync: "merge",
};

test("trims values and accepts a complete form", () => {
  const check = validateSetup({
    ...valid,
    userName: " Alex Ellwein ",
    repoPath: " /Users/alex/bookmarks ",
  });
  assert.equal(check.ok, true);
  if (check.ok) {
    assert.equal(check.form.userName, "Alex Ellwein");
    assert.equal(check.form.repoPath, "/Users/alex/bookmarks");
  }
});

test("reports every missing field at once", () => {
  const check = validateSetup({ ...valid, userName: " ", userEmail: "nope", repoPath: "" });
  assert.equal(check.ok, false);
  if (!check.ok) {
    assert.equal(check.errors.length, 3);
  }
});

test("rejects an unknown initial sync choice", () => {
  const check = validateSetup({ ...valid, initialSync: "both" as "merge" });
  assert.equal(check.ok, false);
});

test("create mode with a remote passes the remote to gitrepo_create", () => {
  const actions = buildSetupActions({ ...valid, remoteUrl: "git@example.com:me/bm.git" });
  assert.deepEqual(actions, [
    { action: "config_set", params: { key: "user_name", value: "Alex Ellwein" } },
    { action: "config_set", params: { key: "user_email", value: "alex@example.com" } },
    {
      action: "gitrepo_create",
      params: { path: "/Users/alex/bookmarks", remote: "git@example.com:me/bm.git" },
    },
  ]);
});

test("create mode without a remote leaves the remote out", () => {
  const actions = buildSetupActions(valid);
  assert.deepEqual(actions[2], { action: "gitrepo_create", params: { path: "/Users/alex/bookmarks" } });
});

test("existing mode sets repo_path before setting the remote", () => {
  const actions = buildSetupActions({
    ...valid,
    repoMode: "existing",
    remoteUrl: "git@example.com:me/bm.git",
  });
  assert.deepEqual(actions.slice(2), [
    { action: "config_set", params: { key: "repo_path", value: "/Users/alex/bookmarks" } },
    { action: "gitrepo_set_remote", params: { url: "git@example.com:me/bm.git" } },
  ]);
});

test("existing mode without a remote does not touch the remote", () => {
  const actions = buildSetupActions({ ...valid, repoMode: "existing" });
  assert.equal(actions.length, 3);
  assert.equal(actions[2].action, "config_set");
});

test("preselects existing when a repo folder is configured", () => {
  assert.equal(initialRepoMode("/Users/alex/git/bm"), "existing");
});

test("preselects create when no repo folder is configured", () => {
  assert.equal(initialRepoMode(null), "create");
  assert.equal(initialRepoMode(""), "create");
});

