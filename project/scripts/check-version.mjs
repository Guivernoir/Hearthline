import { execFileSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "../..");
const expected = (
  await readFile(resolve(root, "project/VERSION"), "utf8")
).trim();

if (!/^(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/.test(expected)) {
  fail(`VERSION is not valid SemVer: ${expected}`);
}

const cargoMetadata = JSON.parse(
  execFileSync(
    "cargo",
    ["metadata", "--no-deps", "--format-version", "1"],
    { cwd: resolve(root, "packages"), encoding: "utf8" },
  ),
);
const rustPackages = cargoMetadata.packages.filter((pkg) =>
  pkg.name.startsWith("hearthline-"),
);
for (const pkg of rustPackages) {
  requireVersion(`Rust package ${pkg.name}`, pkg.version);
}

const webPackage = JSON.parse(
  await readFile(resolve(root, "packages/web/package.json"), "utf8"),
);
const webLock = JSON.parse(
  await readFile(resolve(root, "packages/web/package-lock.json"), "utf8"),
);
requireVersion("packages/web/package.json", webPackage.version);
requireVersion("packages/web/package-lock.json", webLock.version);
requireVersion(
  "packages/web/package-lock.json root package",
  webLock.packages?.[""]?.version,
);

process.stdout.write(
  `Hearthline ${expected}: VERSION, ${rustPackages.length} Rust packages, and npm metadata agree.\n`,
);

function requireVersion(source, actual) {
  if (actual !== expected) {
    fail(`${source} uses ${actual ?? "no version"}, expected ${expected}`);
  }
}

function fail(message) {
  process.stderr.write(`${message}\n`);
  process.exit(1);
}
