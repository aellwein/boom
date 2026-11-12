//! Messages exchanged with the boom host over native messaging.
//!
//! The host name must match `name` in the host manifest that the browser
//! reads. Requests and responses are the JSON types of `boom-host`.

import type { TreeNode } from "./model.js";

export const HOST_NAME = "com.boom.host";

export interface PingRequest {
  id: string;
  action: "ping";
}

export interface PingResult {
  version: string;
}

export interface HostResponse<T = unknown> {
  id: string;
  ok: boolean;
  result?: T;
  error?: string;
}

export function pingRequest(id: string): PingRequest {
  return { id, action: "ping" };
}

export type ConfigKey = "user_name" | "user_email" | "repo_path";

/// Every action the host accepts, with its params. Kept in step with the
/// `Action` enum in `boom-host/src/protocol.rs`.
export type HostAction =
  | { action: "ping" }
  | { action: "config_show" }
  | { action: "config_set"; params: { key: ConfigKey; value: string } }
  | { action: "gitrepo_create"; params: { path: string; remote?: string } }
  | { action: "gitrepo_set_remote"; params: { url: string } }
  | { action: "gitrepo_status" }
  | { action: "pull" }
  | { action: "repo_entries" }
  | { action: "repo_head" }
  | { action: "repo_changes_since"; params: { since: string } }
  | { action: "write_browser_tree"; params: { tree: TreeNode[]; expected_head: string } }
  | { action: "sync" };

/// The host's config, as `config_show` returns it.
export interface HostConfig {
  user_name: string | null;
  user_email: string | null;
  repo_path: string | null;
  conflict_policy: string;
  poll_interval_secs: number;
}

/// Result of `pull`, as the host returns it.
export interface PullResult {
  integration: "up_to_date" | "fast_forward" | "merged";
  conflicts: number;
}

/// The last pull, kept in extension storage so the popup can show it.
export interface PullRecord {
  at: string;
  ok: boolean;
  result?: PullResult;
  error?: string;
}

/// One folder or bookmark of the repo, as `repo_entries` returns it.
/// `path` is relative to the repo root, and entries come sorted by path.
export interface RepoEntry {
  path: string;
  kind: "folder" | "bookmark";
  title: string;
  url: string | null;
}

/// The configured repo folder and its `origin` URL, as `gitrepo_status` returns them.
export interface RepoStatus {
  repo_path: string | null;
  origin_url: string | null;
}

/// One change from the repo at an older commit to the working tree, as
/// `repo_changes_since` returns it. Paths use `/` and are repo-relative.
export type RepoChange =
  | { op: "remove_bookmark"; path: string }
  | { op: "remove_folder"; path: string }
  | { op: "create_folder"; path: string; parent: string | null; index: number; title: string }
  | { op: "write_bookmark"; path: string; parent: string | null; index: number; title: string; url: string };

/// Result of `write_browser_tree` and `repo_head`: the repo's HEAD commit.
export interface HeadResult {
  head: string;
}

export function withId(id: string, action: HostAction): HostAction & { id: string } {
  return { id, ...action };
}

/// Checks that a raw reply from the host has the expected shape and id.
/// Throws if it does not, so a broken host never looks like a success.
export function parseResponse<T>(raw: unknown, expectedId: string): HostResponse<T> {
  if (typeof raw !== "object" || raw === null) {
    throw new Error("host reply is not an object");
  }
  const reply = raw as Record<string, unknown>;
  if (reply.id !== expectedId) {
    throw new Error(`host reply has id ${String(reply.id)}, expected ${expectedId}`);
  }
  if (typeof reply.ok !== "boolean") {
    throw new Error("host reply has no boolean ok field");
  }
  if (reply.ok === false && typeof reply.error !== "string") {
    throw new Error("failed host reply has no error message");
  }
  return reply as unknown as HostResponse<T>;
}
