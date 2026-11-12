import { test } from "node:test";
import assert from "node:assert/strict";

import { parseResponse, pingRequest } from "../protocol.js";

test("ping request has the ping action and the given id", () => {
  assert.deepEqual(pingRequest("7"), { id: "7", action: "ping" });
});

test("accepts a successful reply with the matching id", () => {
  const reply = parseResponse<{ version: string }>(
    { id: "1", ok: true, result: { version: "0.0.1" } },
    "1",
  );
  assert.equal(reply.result?.version, "0.0.1");
});

test("accepts a failed reply that carries an error", () => {
  const reply = parseResponse({ id: "1", ok: false, error: "no config" }, "1");
  assert.equal(reply.error, "no config");
});

test("rejects a reply with a different id", () => {
  assert.throws(() => parseResponse({ id: "2", ok: true }, "1"), /expected 1/);
});

test("rejects a non-object reply", () => {
  assert.throws(() => parseResponse("pong", "1"), /not an object/);
  assert.throws(() => parseResponse(null, "1"), /not an object/);
});

test("rejects a reply without a boolean ok", () => {
  assert.throws(() => parseResponse({ id: "1" }, "1"), /boolean ok/);
});

test("rejects a failed reply without an error message", () => {
  assert.throws(() => parseResponse({ id: "1", ok: false }, "1"), /no error message/);
});
