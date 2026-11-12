//! Calls the boom host through the browser's native messaging API.

import { HOST_NAME } from "./protocol.js";

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
