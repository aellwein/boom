//! Sends browser bookmark changes to the repo.
//!
//! Only runs after the first sync has happened (`idMap` exists), and only
//! while HEAD is the commit the browser last saw. Any other change in the
//! repo would be overwritten by a browser snapshot, so the write is refused.
//! Changes coming from the repo into the browser are not applied yet.

import { callHost } from "./host.js";
import { ID_MAP_KEY, LAST_HEAD_KEY, LAST_WRITE_KEY, SUSPEND_KEY } from "./keys.js";
import { repoTreeFromBrowser, type BrowserBookmarkNode } from "./browser-tree.js";
import type { HeadResult } from "./protocol.js";

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
  const gate = gateWrite({
    hasIdMap: stored[ID_MAP_KEY] !== undefined,
    lastSeenHead: stored[LAST_HEAD_KEY] as string | undefined,
    suspended: stored[SUSPEND_KEY] === true,
  });
  if (!gate.ok) {
    return;
  }

  const [root] = await chrome.bookmarks.getTree();
  const tree = repoTreeFromBrowser(root as BrowserBookmarkNode);
  const at = new Date().toISOString();
  try {
    const reply = await callHost<HeadResult>({
      action: "write_browser_tree",
      params: { tree, expected_head: gate.head },
    });
    if (reply.ok && reply.result) {
      await chrome.storage.local.set({
        [LAST_HEAD_KEY]: reply.result.head,
        [LAST_WRITE_KEY]: { at, ok: true },
      });
    } else {
      await chrome.storage.local.set({ [LAST_WRITE_KEY]: { at, ok: false, error: reply.error } });
    }
  } catch (err) {
    await chrome.storage.local.set({ [LAST_WRITE_KEY]: { at, ok: false, error: String(err) } });
  }
}
