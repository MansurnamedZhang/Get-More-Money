import { readFile, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const frontendRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const target = path.join(
  frontendRoot,
  "node_modules",
  "vinext",
  "dist",
  "server",
  "static-file-cache.js",
);

const windowsRelativePath = "relativePath: path.relative(base, batch[j]),";
const portableRelativePath =
  'relativePath: path.relative(base, batch[j]).split(path.sep).join("/"),';
const source = await readFile(target, "utf8");

if (source.includes(portableRelativePath)) {
  console.log("[sanyu] vinext Windows static path patch already applied");
} else if (source.includes(windowsRelativePath)) {
  await writeFile(target, source.replace(windowsRelativePath, portableRelativePath), "utf8");
  console.log("[sanyu] applied vinext Windows static path patch");
} else {
  throw new Error("Unsupported vinext static-file-cache implementation");
}
