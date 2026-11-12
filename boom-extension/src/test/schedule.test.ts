import { test } from "node:test";
import assert from "node:assert/strict";

import { describePull, pollPeriodMinutes, shouldSchedule } from "../schedule.js";
import type { HostConfig } from "../protocol.js";

const config: HostConfig = {
  user_name: "John Doe",
  user_email: "john@example.com",
  repo_path: null,
  conflict_policy: "last_writer_wins",
  poll_interval_secs: 300,
};

test("converts seconds to minutes", () => {
  assert.equal(pollPeriodMinutes(300), 5);
  assert.equal(pollPeriodMinutes(600), 10);
});

test("keeps at least one minute", () => {
  assert.equal(pollPeriodMinutes(30), 1);
});

test("falls back to five minutes for an unusable interval", () => {
  assert.equal(pollPeriodMinutes(0), 5);
  assert.equal(pollPeriodMinutes(Number.NaN), 5);
});

test("schedules only when a repo path is configured", () => {
  assert.equal(shouldSchedule(config), false);
  assert.equal(shouldSchedule({ ...config, repo_path: "" }), false);
  assert.equal(shouldSchedule({ ...config, repo_path: "/Users/john/bookmarks" }), true);
});

test("describes a successful pull", () => {
  assert.equal(
    describePull({ at: "t", ok: true, result: { integration: "up_to_date", conflicts: 0 } }),
    "Last pull: up to date",
  );
  assert.equal(
    describePull({ at: "t", ok: true, result: { integration: "fast_forward", conflicts: 0 } }),
    "Last pull: updated from origin",
  );
  assert.equal(
    describePull({ at: "t", ok: true, result: { integration: "merged", conflicts: 2 } }),
    "Last pull: merged, 2 conflict(s) resolved by last writer wins",
  );
});

test("describes a failed pull with its error", () => {
  assert.equal(
    describePull({ at: "t", ok: false, error: "uncommitted changes" }),
    "Last pull failed: uncommitted changes",
  );
});
