//! Popup: shows whether the boom host is reachable.

import type { HostResponse, PingResult } from "./protocol.js";

const status = document.getElementById("status") as HTMLParagraphElement;
const button = document.getElementById("ping") as HTMLButtonElement;

async function checkHost(): Promise<void> {
  status.textContent = "Checking host...";
  const reply = (await chrome.runtime.sendMessage({ type: "ping" })) as HostResponse<PingResult>;
  if (reply.ok && reply.result) {
    status.textContent = `Host ${reply.result.version} is running.`;
  } else {
    status.textContent = `Host error: ${reply.error ?? "unknown"}`;
  }
}

button.addEventListener("click", () => {
  void checkHost();
});
void checkHost();
