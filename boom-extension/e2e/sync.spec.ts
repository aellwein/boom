// End-to-end scenarios: a real Chromium with the built extension, the real boom
// host, and a local bare git remote. See fixtures.ts for the setup.

import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { expect, git, test, type BoomEnv } from "./fixtures";

const BOOKMARK = { title: "E2E from Chrome", url: "https://example.com/e2e-from-chrome" };
const REMOTE_BOOKMARK = { title: "E2E from remote", url: "https://example.com/e2e-from-remote" };

/** Fills in the setup page and saves it. Overwrite replaces the browser's bookmarks afterwards. */
async function setup(
  boom: BoomEnv,
  { mode, initialSync }: { mode: "create" | "existing"; initialSync: "merge" | "overwrite" },
): Promise<void> {
  const page = await boom.openExtensionPage("setup.html");
  await expect(page.locator("#host-status")).toHaveText(/^Connected/);
  await page.getByLabel("Name", { exact: true }).fill("John Doe");
  await page.getByLabel("Email", { exact: true }).fill("john@example.com");
  await page.getByLabel("Folder", { exact: true }).fill(boom.repo);
  await page.getByLabel("Remote URL (optional)").fill(boom.remote);
  await page.locator(`input[name=repoMode][value=${mode}]`).check();
  await page.locator(`input[name=initialSync][value=${initialSync}]`).check();
  await page.getByRole("button", { name: "Save setup" }).click();
  await expect(page.locator("#message")).toHaveText("Setup saved.");

  if (initialSync === "overwrite") {
    page.once("dialog", (dialog) => dialog.accept());
    await page.locator("#overwrite").click();
    await expect(page.locator("#message")).toHaveText(/^Replaced/);
  }
  await page.close();
}

/** Waits until the first sync has built the bookmark id map. */
async function firstSync(boom: BoomEnv): Promise<void> {
  await boom.syncNow();
  await expect
    .poll(() => boom.worker.evaluate(() => chrome.storage.local.get("idMap").then((r) => r.idMap !== undefined)))
    .toBe(true);
}

/** True when the branch on the bare remote contains the text. */
function remoteContains(boom: BoomEnv, text: string): boolean {
  const branch = remoteBranch(boom);
  if (branch === undefined) return false;
  try {
    git(["--git-dir", boom.remote, "grep", "-q", "-F", text, branch]);
    return true;
  } catch {
    return false;
  }
}

function remoteBranch(boom: BoomEnv): string | undefined {
  const branches = git(["--git-dir", boom.remote, "for-each-ref", "--format=%(refname:short)", "refs/heads"]);
  return branches.split("\n")[0] || undefined;
}

async function countBookmarks(boom: BoomEnv, url: string): Promise<number> {
  return boom.worker.evaluate((u) => chrome.bookmarks.search({ url: u }).then((r) => r.length), url);
}

test("a bookmark created in Chrome is committed and pushed to the remote", async ({ boom }) => {
  await setup(boom, { mode: "create", initialSync: "merge" });
  await firstSync(boom);

  await boom.worker.evaluate(
    (b) => chrome.bookmarks.create({ parentId: "1", title: b.title, url: b.url }),
    BOOKMARK,
  );

  await expect.poll(() => remoteContains(boom, BOOKMARK.url)).toBe(true);
});

test("a bookmark pushed from another clone appears in Chrome after Sync now", async ({ boom }) => {
  await setup(boom, { mode: "create", initialSync: "merge" });
  await firstSync(boom);

  // Simulates another machine: clone, add a bookmark file, push.
  const branch = remoteBranch(boom);
  expect(branch).toBeDefined();
  const other = `${boom.repo}-other`;
  git(["clone", "-q", "-b", branch!, boom.remote, other]);
  // A repo with no bookmarks has no "other" folder yet, so create it.
  mkdirSync(join(other, "0020_other"), { recursive: true });
  writeFileSync(
    join(other, "0020_other", "0020_E2E from remote.toml"),
    `title = "${REMOTE_BOOKMARK.title}"\nurl = "${REMOTE_BOOKMARK.url}"\n`,
  );
  git(["add", "-A"], other);
  git(
    [
      "-c", "user.name=John Doe",
      "-c", "user.email=john@example.com",
      "-c", "commit.gpgsign=false",
      "commit", "-q", "-m", "test: bookmark from another machine",
    ],
    other,
  );
  git(["push", "-q", "origin", branch!], other);

  await boom.syncNow();
  await expect.poll(() => countBookmarks(boom, REMOTE_BOOKMARK.url)).toBe(1);
});

test("overwrite replaces browser-only bookmarks with the repo's", async ({ boom }) => {
  await setup(boom, { mode: "create", initialSync: "merge" });
  await firstSync(boom);
  await boom.worker.evaluate(
    (b) => chrome.bookmarks.create({ parentId: "1", title: b.title, url: b.url }),
    BOOKMARK,
  );
  await expect.poll(() => remoteContains(boom, BOOKMARK.url)).toBe(true);

  // Writes are paused so the browser-only bookmark is not sent to the repo.
  await boom.worker.evaluate(() => chrome.storage.local.set({ suspendBrowserWrites: true }));
  const localOnly = { title: "Only in browser", url: "https://example.com/only-in-browser" };
  await boom.worker.evaluate(
    (b) => chrome.bookmarks.create({ parentId: "1", title: b.title, url: b.url }),
    localOnly,
  );

  await setup(boom, { mode: "existing", initialSync: "overwrite" });

  expect(await countBookmarks(boom, localOnly.url)).toBe(0);
  expect(await countBookmarks(boom, BOOKMARK.url)).toBe(1);
});
