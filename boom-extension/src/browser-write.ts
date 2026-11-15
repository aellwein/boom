//! Sends browser bookmark changes to the repo.
//!
//! Only runs after the first sync has happened (`idMap` exists), and only
//! while HEAD is the commit the browser last saw. Any other change in the
//! repo would be overwritten by a browser snapshot, so the write is refused.
//! Changes coming from the repo into the browser are not applied yet.

import { callHost } from "./host.js";
import { ID_MAP_KEY, LAST_HEAD_KEY, LAST_WRITE_KEY, SUSPEND_KEY } from "./keys.js";
import { repoTreeFromBrowser, type BrowserBookmarkNode } from "./browser-tree.js";
import { buildIdMap, repoHasBookmarks } from "./id-map.js";
import type { HeadResult, RepoEntry } from "./protocol.js";

/// Pause after the last browser change before writing, so a burst of edits
/// is sent once.
const DEBOUNCE_MS = 2000;

export interface WriteState {
  hasIdMap: boolean;
  lastSeenHead?: string;
  suspended: boolean;
}

export type WriteGate = { ok: true; head: string } | { ok: false; reason: string };

/// Decides whether a browser write may run now, and against which head.
export function gateWrite(state: WriteState): WriteGate {
  if (state.suspended) {
    return { ok: false, reason: "an overwrite is running" };
  }
  if (!state.hasIdMap) {
    return { ok: false, reason: "the first sync has not run" };
  }
  if (state.lastSeenHead === undefined || state.lastSeenHead === "") {
    return { ok: false, reason: "the repo state is not known" };
  }
  return { ok: true, head: state.lastSeenHead };
}

export interface WriteRecord {
  at: string;
  ok: boolean;
  error?: string;
}

/// One line for the popup, describing the last browser write.
export function describeWrite(record: WriteRecord): string {
  return record.ok ? "Last browser write: saved to the repo" : `Last browser write failed: ${record.error ?? "unknown error"}`;
}

let timer: ReturnType<typeof setTimeout> | undefined;
let running = false;
let again = false;

/// Called on every bookmark change. Writes after a pause.
export function scheduleBrowserWrite(): void {
  if (timer !== undefined) {
    clearTimeout(timer);
  }
  timer = setTimeout(() => {
    timer = undefined;
    void writeBrowserChanges();
  }, DEBOUNCE_MS);
}

/// Sends the browser's current bookmarks to the repo, if the gate allows it.
export async function writeBrowserChanges(): Promise<void> {
  if (running) {
    again = true;
    return;
  }
  running = true;
  try {
    do {
      again = false;
      await writeOnce();
    } while (again);
  } finally {
    running = false;
  }
}

async function writeOnce(): Promise<void> {
  const stored = await chrome.storage.local.get([ID_MAP_KEY, LAST_HEAD_KEY, SUSPEND_KEY]);
  if (stored[SUSPEND_KEY] === true) {
    return;
  }
  if (stored[ID_MAP_KEY] === undefined) {
    await firstWrite();
    return;
  }
  const gate = gateWrite({
    hasIdMap: true,
    lastSeenHead: stored[LAST_HEAD_KEY] as string | undefined,
    suspended: false,
  });
  if (!gate.ok) {
    return;
  }
  const head = await sendBrowserTree(gate.head);
  if (head.ok) {
    await rebuildIdMap(head.head);
  }
}

/// The first write, before any ID map exists. It is allowed only into a repo
/// with no bookmarks, so nothing in the repo can be lost.
async function firstWrite(): Promise<void> {
  const entries = await callHost<RepoEntry[]>({ action: "repo_entries" });
  if (!entries.ok || !entries.result) {
    await recordWrite({ ok: false, error: entries.error ?? "could not read the repo" });
    return;
  }
  if (repoHasBookmarks(entries.result)) {
    await recordWrite({
      ok: false,
      error: "the repo already has bookmarks; use Replace browser bookmarks on the setup page first",
    });
    return;
  }
  const head = await callHost<HeadResult>({ action: "repo_head" });
  if (!head.ok || !head.result) {
    await recordWrite({ ok: false, error: head.error ?? "could not read the repo head" });
    return;
  }
  const written = await sendBrowserTree(head.result.head);
  if (written.ok) {
    await rebuildIdMap(written.head);
  }
}

/// Sends the browser's tree to the repo. Records the outcome. On success
/// returns the new head.
async function sendBrowserTree(expectedHead: string): Promise<{ ok: true; head: string } | { ok: false }> {
  const [root] = await chrome.bookmarks.getTree();
  const tree = repoTreeFromBrowser(root as BrowserBookmarkNode);
  const at = new Date().toISOString();
  try {
    const reply = await callHost<HeadResult>({
      action: "write_browser_tree",
      params: { tree, expected_head: expectedHead },
    });
    if (reply.ok && reply.result) {
      await chrome.storage.local.set({
        [LAST_HEAD_KEY]: reply.result.head,
        [LAST_WRITE_KEY]: { at, ok: true },
      });
      return { ok: true, head: reply.result.head };
    }
    await recordWrite({ at, ok: false, error: reply.error });
  } catch (err) {
    await recordWrite({ at, ok: false, error: String(err) });
  }
  return { ok: false };
}

/// Rebuilds the ID map from the repo after a successful write. If the two do
/// not line up, the map is removed, so later writes stop with an error rather
/// than work from stale IDs.
async function rebuildIdMap(head: string): Promise<void> {
  const entries = await callHost<RepoEntry[]>({ action: "repo_entries" });
  const [root] = await chrome.bookmarks.getTree();
  const pairing = entries.ok && entries.result ? buildIdMap(root as BrowserBookmarkNode, entries.result) : undefined;
  if (pairing?.ok) {
    await chrome.storage.local.set({ [ID_MAP_KEY]: pairing.idMap, [LAST_HEAD_KEY]: head });
    return;
  }
  await chrome.storage.local.remove(ID_MAP_KEY);
  await recordWrite({
    ok: false,
    error: `saved to the repo, but the ID map could not be rebuilt: ${pairing && !pairing.ok ? pairing.reason : entries.error ?? "unknown"}`,
  });
}

async function recordWrite(record: { at?: string; ok: boolean; error?: string }): Promise<void> {
  await chrome.storage.local.set({ [LAST_WRITE_KEY]: { at: record.at ?? new Date().toISOString(), ok: record.ok, error: record.error } });
}
