import { readFileSync, writeFileSync } from "fs";
import { resolve, dirname } from "path";
import { fileURLToPath } from "url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

const cargo = readFileSync(resolve(root, "src-tauri/Cargo.toml"), "utf8");
const match = cargo.match(/^version\s*=\s*"([^"]+)"/m);
if (!match) {
  console.error("Could not find version in Cargo.toml");
  process.exit(1);
}
const version = match[1];

const pkgPath = resolve(root, "package.json");
const pkg = JSON.parse(readFileSync(pkgPath, "utf8"));
if (pkg.version !== version) {
  pkg.version = version;
  writeFileSync(pkgPath, JSON.stringify(pkg, null, 2) + "\n");
  console.log(`Synced version to ${version}`);
}
