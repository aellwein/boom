import { test } from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

// Tests run from dist/test, so the repo root is three levels up.
const repo = join(dirname(fileURLToPath(import.meta.url)), "..", "..", "..");
const read = (path: string) => JSON.parse(readFileSync(join(repo, path), "utf8"));

test("the Firefox manifest's Gecko ID is the one the Firefox host allows", () => {
  const manifest = read("boom-extension/src/manifest.firefox.json");
  const host = read("native-host/firefox/com.boom.host.json");
  assert.deepEqual(host.allowed_extensions, [manifest.browser_specific_settings.gecko.id]);
});

test("the host manifests share the name the extensions connect to", () => {
  assert.equal(read("native-host/firefox/com.boom.host.json").name, "com.boom.host");
  assert.equal(read("native-host/chrome/com.boom.host.json").name, "com.boom.host");
});

test("Firefox uses its own host manifest, not the Chromium one", () => {
  const host = read("native-host/chrome/com.boom.host.json");
  assert.equal(host.allowed_extensions, undefined);
  assert.ok(Array.isArray(host.allowed_origins));
});
