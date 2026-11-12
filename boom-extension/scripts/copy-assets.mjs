// Copies the extension files for one browser into its own folder, so each
// folder is a complete unpacked extension:
//   dist/          Chromium (Chrome, Edge, Brave, Opera)
//   dist-firefox/  Firefox
// The compiled scripts are in dist/ (see tsconfig.json).
import { copyFile, mkdir, readdir } from "node:fs/promises";
import { join } from "node:path";

const targets = {
  chromium: { out: "dist", manifest: "manifest.json" },
  firefox: { out: "dist-firefox", manifest: "manifest.firefox.json" },
};

const target = process.argv[2] ?? "chromium";
const config = targets[target];
if (!config) {
  throw new Error(`unknown target ${target}; use one of ${Object.keys(targets).join(", ")}`);
}

const root = new URL("..", import.meta.url).pathname;
const src = join(root, "src");
const dist = join(root, "dist");
const out = join(root, config.out);
const staticAssets = ["popup.html", "popup.css", "setup.html", "setup.css"];

await mkdir(out, { recursive: true });
await copyFile(join(src, config.manifest), join(out, "manifest.json"));
for (const name of staticAssets) {
  await copyFile(join(src, name), join(out, name));
}

// Firefox keeps its own folder, so the compiled scripts are copied there too.
if (out !== dist) {
  for (const name of await readdir(dist)) {
    if (name.endsWith(".js")) {
      await copyFile(join(dist, name), join(out, name));
    }
  }
}
