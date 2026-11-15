// Shared test fixtures. Each test gets its own temp HOME, Chromium profile,
// bare git remote and bookmark repo. Nothing touches the user's Chrome
// profile, home directory or GitHub.

import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { chromium, expect, test as base, type BrowserContext, type Page, type Worker } from "@playwright/test";

const here = dirname(fileURLToPath(import.meta.url));
export const EXTENSION_DIR = resolve(here, "..", "dist");
export const BOOM_BIN = process.env.BOOM_BIN ?? resolve(here, "..", "..", "target", "debug", "boom");

export interface BoomEnv {
  /** Extension service worker: the place where chrome.bookmarks and chrome.storage run. */
  worker: Worker;
  /** Extension id, the same as the chrome-extension:// host of the worker. */
  extensionId: string;
  context: BrowserContext;
  /** Bare git remote the host pushes to. */
  remote: string;
  /** Bookmark repo the host works in. */
  repo: string;
  /** Opens an extension page (setup.html, popup.html, ...) in a new tab. */
  openExtensionPage(path: string): Promise<Page>;
  /** Runs `git` in the given directory and returns trimmed stdout. */
  git(args: string[], cwd?: string): string;
  /** Sends the popup's "Sync now" message from an extension page. */
  syncNow(): Promise<void>;
}

export function git(args: string[], cwd?: string): string {
  return execFileSync("git", args, { cwd, encoding: "utf8" }).trim();
}

export const test = base.extend<{ boom: BoomEnv }>({
  boom: async ({}, use, testInfo) => {
    const root = mkdtempSync(join(tmpdir(), "boom-pw-"));
    const home = join(root, "home");
    const profile = join(root, "profile");
    const remote = join(root, "remote.git");
    const repo = join(root, "bookmarks");
    mkdirSync(home);
    git(["init", "--bare", "-q", remote]);

    // Chromium reads native host manifests from the profile's NativeMessagingHosts
    // folder (verified on macOS). The host inherits HOME, so its config lands in
    // the temp home.
    const context = await chromium.launchPersistentContext(profile, {
      channel: "chromium",
      headless: !process.env.HEADED,
      env: { ...process.env, HOME: home },
      ignoreDefaultArgs: ["--disable-extensions"],
      args: [
        `--disable-extensions-except=${EXTENSION_DIR}`,
        `--load-extension=${EXTENSION_DIR}`,
        "--use-mock-keychain",
        "--no-first-run",
        "--no-default-browser-check",
      ],
    });

    try {
      const worker = await findBoomWorker(context);
      const extensionId = new URL(worker.url()).host;
      installNativeHostManifest(profile, extensionId);

      const env: BoomEnv = {
        worker,
        extensionId,
        context,
        remote,
        repo,
        git,
        async openExtensionPage(path) {
          const page = await context.newPage();
          await page.goto(`chrome-extension://${extensionId}/${path}`);
          return page;
        },
        async syncNow() {
          const page = await env.openExtensionPage("popup.html");
          try {
            await page.evaluate(() => chrome.runtime.sendMessage({ type: "sync_now" }));
          } finally {
            await page.close();
          }
        },
      };
      await use(env);
    } finally {
      await context.close();
      if (process.env.KEEP_E2E !== "1") {
        rmSync(root, { recursive: true, force: true });
      } else {
        console.log(`kept ${root} for ${testInfo.title}`);
      }
    }
  },
});

export { expect };

/// Finds the boom service worker. Chromium also runs its own extensions, so the
/// worker is picked by its manifest name.
async function findBoomWorker(context: BrowserContext): Promise<Worker> {
  const deadline = Date.now() + 30_000;
  while (Date.now() < deadline) {
    for (const worker of context.serviceWorkers()) {
      const name = await worker.evaluate(() => chrome.runtime.getManifest?.().name).catch(() => undefined);
      if (name === "boom") return worker;
    }
    await context.waitForEvent("serviceworker", { timeout: 1000 }).catch(() => undefined);
  }
  throw new Error("the boom service worker did not start");
}

function installNativeHostManifest(profileDir: string, extensionId: string): void {
  const dir = join(profileDir, "NativeMessagingHosts");
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "com.boom.host.json"),
    JSON.stringify(
      {
        name: "com.boom.host",
        description: "boom native messaging host (e2e test)",
        path: BOOM_BIN,
        type: "stdio",
        allowed_origins: [`chrome-extension://${extensionId}/`],
      },
      null,
      2,
    ),
  );
}
