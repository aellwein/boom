//! Keys of extension storage (`chrome.storage.local`). Kept in one module so
//! the modules that share state do not import each other.

/// Repo path to browser bookmark ID, for every synced folder and bookmark.
export const ID_MAP_KEY = "idMap";
/// Repo HEAD commit the browser last mirrored.
export const LAST_HEAD_KEY = "lastSeenHead";
/// True while an overwrite runs, so its own changes are not sent back.
export const SUSPEND_KEY = "suspendBrowserWrites";
/// Outcome of the last browser write, for the popup.
export const LAST_WRITE_KEY = "lastBrowserWrite";
