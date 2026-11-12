//! Calls the boom host through the browser's native messaging API.

import { HOST_NAME, parseResponse, withId, type HostAction, type HostResponse } from "./protocol.js";

let nextId = 0;

/// Sends one message to the host and resolves with its raw reply.
/// Each call starts the host, so the host keeps no state between calls.
export function sendToHost(message: object): Promise<unknown> {
  return new Promise((resolve, reject) => {
    chrome.runtime.sendNativeMessage(HOST_NAME, message, (reply: unknown) => {
      const error = chrome.runtime.lastError;
      if (error) {
        reject(new Error(error.message));
      } else {
        resolve(reply);
      }
    });
  });
}

/// Sends one action to the host and returns its validated reply.
export async function callHost<T = unknown>(action: HostAction): Promise<HostResponse<T>> {
  const id = String(++nextId);
  const raw = await sendToHost(withId(id, action));
  return parseResponse<T>(raw, id);
}
