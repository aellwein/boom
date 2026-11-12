//! Service worker. Answers requests from the popup by calling the host.

import { parseResponse, pingRequest, type HostResponse, type PingResult } from "./protocol.js";
import { sendToHost } from "./host.js";

let nextId = 0;

chrome.runtime.onMessage.addListener((message: unknown, _sender, sendResponse) => {
  if (typeof message !== "object" || message === null || (message as { type?: unknown }).type !== "ping") {
    return false;
  }

  const id = String(++nextId);
  sendToHost(pingRequest(id))
    .then((raw) => sendResponse(parseResponse<PingResult>(raw, id)))
    .catch((err: unknown) => {
      const failure: HostResponse = { id, ok: false, error: String(err) };
      sendResponse(failure);
    });
  // Returning true keeps the message channel open for the async reply.
  return true;
});
