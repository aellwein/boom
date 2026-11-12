//! Popup: shows whether the boom host is reachable, and a read-only summary
//! of the browser's bookmarks.

import type { BookmarksResult } from "./background.js";
import type { HostResponse, PingResult, PullRecord } from "./protocol.js";
import { describePull, LAST_PULL_KEY } from "./schedule.js";
import { describeWrite, type WriteRecord } from "./browser-write.js";
import { LAST_WRITE_KEY } from "./keys.js";

const status = document.getElementById("status") as HTMLParagraphElement;
const pingButton = document.getElementById("ping") as HTMLButtonElement;
const bookmarksButton = document.getElementById("bookmarks") as HTMLButtonElement;
const bookmarkList = document.getElementById("bookmark-list") as HTMLUListElement;
const lastPull = document.getElementById("last-pull") as HTMLParagraphElement;
const lastWrite = document.getElementById("last-write") as HTMLParagraphElement;

async function checkHost(): Promise<void> {
  status.textContent = "Checking host...";
  const reply = (await chrome.runtime.sendMessage({ type: "ping" })) as HostResponse<PingResult>;
  if (reply.ok && reply.result) {
    status.textContent = `Host ${reply.result.version} is running.`;
  } else {
    status.textContent = `Host error: ${reply.error ?? "unknown"}`;
  }
}

async function showBookmarks(): Promise<void> {
  bookmarkList.replaceChildren();
  const reply = (await chrome.runtime.sendMessage({ type: "bookmarks" })) as
    | { ok: true; result: BookmarksResult }
    | { ok: false; error: string };
  if (!reply.ok) {
    addItem(`Could not read bookmarks: ${reply.error}`);
    return;
  }
  addItem(`${reply.result.summary.bookmarks} bookmarks in ${reply.result.summary.folders} folders`);
  for (const root of reply.result.roots) {
    addItem(`${root.title}: ${root.summary.bookmarks} bookmarks, ${root.summary.folders} folders`);
  }
}

async function showLastPull(): Promise<void> {
  const stored = await chrome.storage.local.get(LAST_PULL_KEY);
  const record = stored[LAST_PULL_KEY] as PullRecord | undefined;
  lastPull.textContent = record ? describePull(record) : "No pull has run yet.";
}

async function showLastWrite(): Promise<void> {
  const stored = await chrome.storage.local.get(LAST_WRITE_KEY);
  const record = stored[LAST_WRITE_KEY] as WriteRecord | undefined;
  lastWrite.textContent = record ? describeWrite(record) : "No browser write has run yet.";
}

function addItem(text: string): void {
  const item = document.createElement("li");
  item.textContent = text;
  bookmarkList.append(item);
}

pingButton.addEventListener("click", () => {
  void checkHost();
void showLastPull();
void showLastWrite();
});
bookmarksButton.addEventListener("click", () => {
  void showBookmarks();
});
void checkHost();
void showLastPull();
void showLastWrite();
