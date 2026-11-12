//! Messages exchanged with the boom host over native messaging.
//!
//! The host name must match `name` in the host manifest that the browser
//! reads. Requests and responses are the JSON types of `boom-host`.

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
  | { action: "sync" };

/// The host's config, as `config_show` returns it.
export interface HostConfig {
  user_name: string | null;
  user_email: string | null;
  repo_path: string | null;
  conflict_policy: string;
  poll_interval_secs: number;
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
