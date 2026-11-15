#!/usr/bin/env node
// End-to-end test: a real Chrome build loads the built extension, the setup
// page drives the real boom host, and the host syncs with a local bare git
// remote. Nothing touches the user's Chrome profile, home directory or GitHub.
//
// Requirements:
//   - `npm run build` in boom-extension (writes dist/)
//   - `cargo build -p boom-host` in the repo root (writes target/debug/boom)
//   - CHROME_FOR_TESTING: path to a Chrome for Testing binary. Branded Google
//     Chrome ignores --load-extension, so it cannot be used. Install one with:
//       npx @puppeteer/browsers install chrome@stable
//     and point the variable at the binary it prints.
//
// Environment:
//   KEEP=1      keep the temp directory for inspection
//   HEADED=1    show the browser window instead of running headless

import { spawn, spawnSync } from "node:child_process";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const extensionDir = resolve(here, "..", "dist");
const boomBin = resolve(here, "..", "..", "target", "debug", "boom");
const chromeBin = process.env.CHROME_FOR_TESTING;

const BOOKMARK = { title: "E2E from Chrome", url: "https://example.com/e2e-from-chrome" };
const TIMEOUT_MS = 30_000;

function fail(message) {
  throw new Error(message);
}

function step(message) {
  console.log(`- ${message}`);
}

function git(args, cwd) {
  const result = spawnSync("git", args, { cwd, encoding: "utf8" });
  if (result.status !== 0) {
    throw new Error(`git ${args.join(" ")} failed: ${result.stderr}`);
  }
  return result.stdout.trim();
}

// The branch the host pushed to: the first branch in the bare remote.
function remoteBranch(remote) {
  const branches = git(["--git-dir", remote, "for-each-ref", "--format=%(refname:short)", "refs/heads"], remote);
  const first = branches.split("\n")[0];
  if (!first) fail("the remote has no branches yet");
  return first;
}

function remoteHasUrl(remote, url) {
  const branches = git(["--git-dir", remote, "for-each-ref", "--format=%(refname:short)", "refs/heads"], remote);
  if (!branches) return false;
  const result = spawnSync("git", ["--git-dir", remote, "grep", "-q", url, remoteBranch(remote)]);
  return result.status === 0;
}

async function waitFor(check, what) {
  const deadline = Date.now() + TIMEOUT_MS;
  while (Date.now() < deadline) {
    const value = await check();
    if (value) return value;
    await new Promise((resolveSleep) => setTimeout(resolveSleep, 500));
  }
  fail(`timed out waiting for ${what}`);
}

// Minimal Chrome DevTools Protocol client over the global WebSocket of Node.
function connectCdp(url) {
  const socket = new WebSocket(url);
  const pending = new Map();
  let nextId = 0;
  socket.onmessage = (event) => {
    const message = JSON.parse(event.data);
    const waiter = pending.get(message.id);
    if (!waiter) return;
    pending.delete(message.id);
    if (message.error) waiter.reject(new Error(JSON.stringify(message.error)));
    else waiter.resolve(message.result);
  };
  const opened = new Promise((resolveOpen, rejectOpen) => {
    socket.onopen = resolveOpen;
    socket.onerror = rejectOpen;
  });
  return {
    opened,
    send(method, params = {}) {
      const id = ++nextId;
      socket.send(JSON.stringify({ id, method, params }));
      return new Promise((resolveSend, rejectSend) => pending.set(id, { resolve: resolveSend, reject: rejectSend }));
    },
    close() {
      socket.close();
    },
  };
}

async function evaluate(cdp, expression) {
  const result = await cdp.send("Runtime.evaluate", {
    expression,
    awaitPromise: true,
    returnByValue: true,
  });
  if (result.exceptionDetails) {
    throw new Error(`evaluate failed: ${JSON.stringify(result.exceptionDetails)}`);
  }
  return result.result.value;
}

async function listTargets(port) {
  const response = await fetch(`http://127.0.0.1:${port}/json/list`);
  return response.json();
}

// HOME is replaced for Chrome, so the native host (which inherits Chrome's
// environment) reads and writes the temp config instead of the real one.
function startChrome(profileDir, home) {
  const args = [
    `--user-data-dir=${profileDir}`,
    "--remote-debugging-port=0",
    "--no-first-run",
    "--no-default-browser-check",
    // Keeps Chrome from using the macOS keychain, which the temp HOME does not have.
    "--use-mock-keychain",
    `--disable-extensions-except=${extensionDir}`,
    `--load-extension=${extensionDir}`,
    "about:blank",
  ];
  if (!process.env.HEADED) args.unshift("--headless=new");
  const child = spawn(chromeBin, args, {
    env: { ...process.env, HOME: home },
    stdio: ["ignore", "ignore", "pipe"],
  });
  child.stderrText = "";
  child.stderr.on("data", (chunk) => {
    child.stderrText += chunk;
  });
  return child;
}

// Chrome for Testing reads native host manifests from NativeMessagingHosts
// inside its user data directory, so the test browser never sees the real
// Library folders. The directory is deleted with the rest of the temp files.
function installNativeHostManifest(profileDir, extensionId) {
  const manifest = {
    name: "com.boom.host",
    description: "boom native messaging host (e2e test)",
    path: boomBin,
    type: "stdio",
    allowed_origins: [`chrome-extension://${extensionId}/`],
  };
  const dir = join(profileDir, "NativeMessagingHosts");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "com.boom.host.json"), JSON.stringify(manifest, null, 2));
}

// Finds the boom service worker. Chrome also runs its own extensions, so the
// worker is picked by its manifest name.
async function findBoomWorker(port) {
  return waitFor(async () => {
    for (const target of (await listTargets(port)).filter((t) => t.type === "service_worker")) {
      const probe = connectCdp(target.webSocketDebuggerUrl);
      await probe.opened;
      const name = await evaluate(probe, "chrome.runtime.getManifest?.().name").catch(() => undefined);
      probe.close();
      if (name === "boom") return target;
    }
    return undefined;
  }, "the boom service worker");
}

// Opens an extension page in a new tab and returns a connected CDP client.
async function openExtensionPage(port, extensionId, path) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?chrome-extension://${extensionId}/${path}`, {
    method: "PUT",
  });
  const target = await response.json();
  const page = connectCdp(target.webSocketDebuggerUrl);
  await page.opened;
  return page;
}

// Sends the same message the popup's "Sync now" button sends.
async function syncNow(port, extensionId) {
  const page = await openExtensionPage(port, extensionId, "popup.html");
  try {
    await waitFor(
      () => evaluate(page, "typeof chrome.runtime.sendMessage === 'function'").catch(() => false),
      "the popup page to load",
    );
    await evaluate(page, "chrome.runtime.sendMessage({ type: 'sync_now' }).then(() => true)");
  } finally {
    page.close();
  }
}

// Drives the setup page the way a user does: fill in the form and save. With
// initialSync "merge" on an empty repo, the first browser write creates the
// bookmark id map. With "overwrite", the setup page's Replace button is pressed
// after saving, which replaces the browser's bookmarks with the repo's.
async function runSetup(port, extensionId, repo, remote, { mode = "create", initialSync = "merge" } = {}) {
  const response = await fetch(`http://127.0.0.1:${port}/json/new?chrome-extension://${extensionId}/setup.html`, {
    method: "PUT",
  });
  const target = await response.json();
  const page = connectCdp(target.webSocketDebuggerUrl);
  await page.opened;
  try {
    await waitFor(
      () => evaluate(page, "document.getElementById('host-status')?.textContent.startsWith('Connected') === true"),
      "the setup page to reach the host",
    );
    await evaluate(
      page,
      `(() => {
        const form = document.getElementById('setup-form');
        const set = (name, value) => { form.elements.namedItem(name).value = value; };
        set('userName', 'John Doe');
        set('userEmail', 'john@example.com');
        set('repoPath', ${JSON.stringify(repo)});
        set('remoteUrl', ${JSON.stringify(remote)});
        form.querySelector('input[name=repoMode][value=${mode}]').checked = true;
        const sync = form.querySelector('input[name=initialSync][value=${initialSync}]');
        sync.checked = true;
        sync.dispatchEvent(new Event('change', { bubbles: true }));
        return true;
      })()`,
    );
    await evaluate(page, "document.getElementById('save').click(), true");
    const saved = await waitFor(
      () =>
        evaluate(
          page,
          "(() => { const t = document.getElementById('message').textContent; return t === 'Setup saved.' || t.startsWith('Setup stopped') ? t : ''; })()",
        ),
      "the setup save to finish",
    );
    if (saved !== "Setup saved.") fail(`setup: ${saved}`);
    step("setup saved");

    if (initialSync === "overwrite") {
      // The page asks for confirmation before replacing bookmarks; accept it.
      await evaluate(page, "window.confirm = () => true; document.getElementById('overwrite').click(), true");
      const replaced = await waitFor(
        () =>
          evaluate(
            page,
            "(() => { const t = document.getElementById('message').textContent; return t.startsWith('Replaced') || t.startsWith('Overwrite stopped') ? t : ''; })()",
          ),
        "the overwrite to finish",
      );
      if (!replaced.startsWith("Replaced")) fail(`overwrite: ${replaced}`);
      step(`overwrite: ${replaced}`);
    }
  } finally {
    page.close();
  }
}

async function main() {
  if (!chromeBin || !existsSync(chromeBin)) {
    throw new Error("set CHROME_FOR_TESTING to a Chrome for Testing binary (see the header of this file)");
  }
  if (!existsSync(join(extensionDir, "manifest.json"))) {
    throw new Error("missing boom-extension/dist; run `npm run build` in boom-extension");
  }
  if (!existsSync(boomBin)) {
    throw new Error("missing target/debug/boom; run `cargo build -p boom-host` in the repo root");
  }

  const root = mkdtempSync(join(tmpdir(), "boom-e2e-"));
  const home = join(root, "home");
  const profile = join(root, "profile");
  const remote = join(root, "remote.git");
  const repo = join(root, "bookmarks");
  mkdirSync(home);
  console.log(`e2e root: ${root}`);

  let chrome;
  let cdp;
  try {
    step("create bare remote");
    git(["init", "--bare", "-q", remote], root);

    step("start Chrome for Testing with the extension");
    chrome = startChrome(profile, home);
    const portFile = join(profile, "DevToolsActivePort");
    const port = await waitFor(
      () => existsSync(portFile) && readFileSync(portFile, "utf8").split("\n")[0],
      "DevTools port",
    );
    const worker = await findBoomWorker(port);
    const extensionId = new URL(worker.url).host;
    step(`extension id ${extensionId}`);
    installNativeHostManifest(profile, extensionId);

    cdp = connectCdp(worker.webSocketDebuggerUrl);
    await cdp.opened;

    await runSetup(port, extensionId, repo, remote);
    if (!existsSync(join(home, ".config", "boom", "config.toml"))) {
      fail("the host did not write its config under the temp HOME");
    }

    step("run Sync now (first sync builds the bookmark id map)");
    await syncNow(port, extensionId);
    await waitFor(async () => (await evaluate(cdp, "chrome.storage.local.get('idMap')")).idMap !== undefined, "the id map");

    step("create a bookmark in Chrome");
    await evaluate(cdp, `chrome.bookmarks.create(${JSON.stringify(BOOKMARK)})`);

    step("wait for the host to commit and push it");
    await waitFor(() => remoteHasUrl(remote, BOOKMARK.url), "the bookmark to reach the remote");

    const log = git(["--git-dir", remote, "log", "--oneline", "-5", remoteBranch(remote)], root);
    console.log(`remote log:\n${log}`);
    console.log("PASS: bookmark created in Chrome reached the remote repo");

    step("push a bookmark from another machine to the remote");
    const branch = remoteBranch(remote);
    const other = join(root, "other");
    git(["clone", "-q", "-b", branch, remote, other], root);
    const remoteBookmark = { title: "E2E from remote", url: "https://example.com/e2e-from-remote" };
    writeFileSync(
      join(other, "0020_other", "0020_E2E from remote.toml"),
      `title = "${remoteBookmark.title}"\nurl = "${remoteBookmark.url}"\n`,
    );
    git(["add", "-A"], other);
    // Signing is off only for this throwaway clone, which has no signing key.
    git(
      [
        "-c", "user.name=John Doe",
        "-c", "user.email=john@example.com",
        "-c", "commit.gpgsign=false",
        "commit", "-q", "-m", "test: bookmark from another machine",
      ],
      other,
    );
    git(["push", "-q", "origin", branch], other);

    step("run Sync now and wait for the bookmark to appear in Chrome");
    await syncNow(port, extensionId);
    await waitFor(
      async () => (await evaluate(cdp, `chrome.bookmarks.search({ url: ${JSON.stringify(remoteBookmark.url)} })`)).length > 0,
      "the remote bookmark to appear in Chrome",
    );
    console.log("PASS: bookmark pushed to the remote appeared in Chrome");

    step("overwrite: a browser-only bookmark is replaced by the repo's bookmarks");
    // Writes are paused so the browser-only bookmark is not sent to the repo.
    await evaluate(cdp, "chrome.storage.local.set({ suspendBrowserWrites: true })");
    const localOnly = { title: "Only in browser", url: "https://example.com/only-in-browser" };
    await evaluate(cdp, `chrome.bookmarks.create({ parentId: '1', title: ${JSON.stringify(localOnly.title)}, url: ${JSON.stringify(localOnly.url)} })`);
    await runSetup(port, extensionId, repo, remote, { mode: "existing", initialSync: "overwrite" });
    const countFor = (url) => evaluate(cdp, `chrome.bookmarks.search({ url: ${JSON.stringify(url)} }).then((r) => r.length)`);
    if ((await countFor(localOnly.url)) !== 0) fail("browser-only bookmark survived the overwrite");
    for (const url of [BOOKMARK.url, remoteBookmark.url]) {
      if ((await countFor(url)) !== 1) fail(`repo bookmark ${url} is missing after the overwrite`);
    }
    console.log("PASS: overwrite replaced the browser's bookmarks with the repo's");
  } catch (err) {
    const dump = cdp
      ? await evaluate(cdp, "chrome.storage.local.get(null)").catch((e) => `storage unavailable: ${e.message}`)
      : "n/a";
    console.error(`storage: ${JSON.stringify(dump, null, 2)}`);
    const ping = cdp
      ? await evaluate(
          cdp,
          "new Promise((resolve) => chrome.runtime.sendNativeMessage('com.boom.host', { id: 'ping', action: 'ping' }, (reply) => resolve(JSON.stringify({ reply, error: chrome.runtime.lastError?.message ?? null }))))",
        ).catch((e) => `ping unavailable: ${e.message}`)
      : "n/a";
    console.error(`native ping from the worker: ${ping}`);
    if (chrome?.stderrText) console.error(`chrome stderr (tail):\n${chrome.stderrText.slice(-2000)}`);
    console.error(`FAIL: ${err.message}`);
    process.exitCode = 1;
  } finally {
    cdp?.close();
    chrome?.kill();
    if (process.env.KEEP) {
      console.log(`kept ${root}`);
    } else {
      rmSync(root, { recursive: true, force: true });
    }
  }
}

main();
