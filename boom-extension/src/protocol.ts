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
