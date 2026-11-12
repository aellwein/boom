//! Service worker. Answers requests from the popup: host ping and bookmark
//! tree. The bookmark request is read-only; nothing is written to the
//! browser or the repo yet.

import type { HostResponse, PingResult } from "./protocol.js";
import { callHost } from "./host.js";
import { PULL_ALARM, runPull, schedulePull } from "./schedule.js";
import { convertBrowserTree } from "./browser-tree.js";
import { summarize, type TreeNode, type TreeSummary } from "./model.js";

export interface BookmarksResult {
  summary: TreeSummary;
  roots: { title: string; summary: TreeSummary; tree: TreeNode }[];
}

chrome.runtime.onMessage.addListener((message: unknown, _sender, sendResponse) => {
  const type = (message as { type?: unknown } | null)?.type;
  if (type === "ping") {
    callHost<PingResult>({ action: "ping" })
      .then((reply) => sendResponse(reply))
      .catch((err: unknown) => {
        const failure: HostResponse = { id: "", ok: false, error: String(err) };
        sendResponse(failure);
      });
    // Returning true keeps the message channel open for the async reply.
    return true;
  }
  if (type === "bookmarks") {
    readBookmarks()
      .then((result) => sendResponse({ ok: true, result }))
      .catch((err: unknown) => sendResponse({ ok: false, error: String(err) }));
    return true;
  }
  return false;
});

async function readBookmarks(): Promise<BookmarksResult> {
  const [root] = await chrome.bookmarks.getTree();
  const roots = convertBrowserTree(root);
  const perRoot = roots.map((tree) => ({ title: tree.title, summary: summarize([tree]), tree }));
  return { summary: summarize(roots), roots: perRoot };
}

// Alarms are re-created on install and browser start. The setup page calls
// schedulePull again after it saves a repo.
chrome.runtime.onInstalled.addListener(() => {
  void schedulePull();
});
chrome.runtime.onStartup.addListener(() => {
  void schedulePull();
});
chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name === PULL_ALARM) {
    void runPull();
  }
});

