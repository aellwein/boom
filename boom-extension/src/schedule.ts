//! Periodic pull. A `chrome.alarms` alarm runs `pull` on the interval set in
//! the host config. Alarms survive service worker restarts, so they are set
//! again on install, on browser start, and after the setup page saves.

import { callHost } from "./host.js";
import { applyRepoToBrowser } from "./repo-apply.js";
import type { HostConfig, PullRecord, PullResult } from "./protocol.js";

export const PULL_ALARM = "boom-pull";
export const LAST_PULL_KEY = "lastPull";

/// Default when the host config holds no usable interval (5 minutes).
const FALLBACK_MINUTES = 5;

/// Converts the host's interval in seconds to alarm minutes, at least one.
export function pollPeriodMinutes(intervalSecs: number): number {
  if (!Number.isFinite(intervalSecs) || intervalSecs <= 0) {
    return FALLBACK_MINUTES;
  }
  return Math.max(1, Math.round(intervalSecs / 60));
}

/// Pulling needs a configured repo. Without one, there is nothing to pull.
export function shouldSchedule(config: HostConfig): boolean {
  return config.repo_path !== null && config.repo_path !== "";
}

/// One line for the popup, describing the last pull.
export function describePull(record: PullRecord): string {
  if (!record.ok || !record.result) {
    return `Last pull failed: ${record.error ?? "unknown error"}`;
  }
  return `Last pull: ${describeResult(record.result)}`;
}

function describeResult(result: PullResult): string {
  switch (result.integration) {
    case "up_to_date":
      return "up to date";
    case "fast_forward":
      return "updated from origin";
    case "merged":
      return result.conflicts === 0
        ? "merged with origin"
        : `merged, ${result.conflicts} conflict(s) resolved by last writer wins`;
  }
}

/// Sets the alarm from the host config, or clears it if no repo is set.
export async function schedulePull(): Promise<void> {
  const reply = await callHost<HostConfig>({ action: "config_show" });
  if (!reply.ok || !reply.result || !shouldSchedule(reply.result)) {
    await chrome.alarms.clear(PULL_ALARM);
    return;
  }
  await chrome.alarms.create(PULL_ALARM, {
    periodInMinutes: pollPeriodMinutes(reply.result.poll_interval_secs),
  });
}

/// Runs one pull, applies what it brought to the browser, and records the
/// outcome for the popup.
export async function runPull(): Promise<void> {
  const at = new Date().toISOString();
  try {
    const reply = await callHost<PullResult>({ action: "pull" });
    const record: PullRecord = { at, ok: reply.ok, result: reply.result, error: reply.error };
    if (reply.ok) {
      try {
        await applyRepoToBrowser();
      } catch (err) {
        record.ok = false;
        record.error = `applying to the browser failed: ${String(err)}`;
      }
    }
    await chrome.storage.local.set({ [LAST_PULL_KEY]: record });
  } catch (err) {
    const record: PullRecord = { at, ok: false, error: String(err) };
    await chrome.storage.local.set({ [LAST_PULL_KEY]: record });
  }
}
