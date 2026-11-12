//! Popup: shows whether the boom host is reachable, and a read-only summary
//! of the browser's bookmarks.

import type { BookmarksResult } from "./background.js";
import type { HostResponse, PingResult } from "./protocol.js";

const status = document.getElementById("status") as HTMLParagraphElement;
const pingButton = document.getElementById("ping") as HTMLButtonElement;
const bookmarksButton = document.getElementById("bookmarks") as HTMLButtonElement;
const bookmarkList = document.getElementById("bookmark-list") as HTMLUListElement;

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

function addItem(text: string): void {
  const item = document.createElement("li");
  item.textContent = text;
  bookmarkList.append(item);
}

pingButton.addEventListener("click", () => {
  void checkHost();
});
bookmarksButton.addEventListener("click", () => {
  void showBookmarks();
});
void checkHost();
