// Copies the static extension files from src/ to dist/, so dist/ is a
// complete unpacked extension that the browser can load.
import { copyFile, mkdir } from "node:fs/promises";
import { join } from "node:path";

const assets = ["manifest.json", "popup.html", "popup.css", "setup.html", "setup.css"];
const root = new URL("..", import.meta.url).pathname;

await mkdir(join(root, "dist"), { recursive: true });
for (const name of assets) {
  await copyFile(join(root, "src", name), join(root, "dist", name));
}
