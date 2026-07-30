import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const maxLines = 500;
const maxEntries = 7;
const ignoredDirectories = new Set([
  ".git",
  ".svelte-kit",
  "dist",
  "node_modules",
  "target",
]);
const lineExemptFiles = new Set(["Cargo.lock", "package-lock.json"]);
const binaryExtensions = new Set([
  ".gif",
  ".ico",
  ".jpeg",
  ".jpg",
  ".pdf",
  ".png",
  ".webp",
]);
const failures = [];

function displayPath(path) {
  return relative(root, path) || ".";
}

function isIgnoredDirectory(name) {
  return ignoredDirectories.has(name);
}

function inspectTree(directory) {
  const entries = readdirSync(directory, { withFileTypes: true }).filter(
    (entry) => !isIgnoredDirectory(entry.name),
  );

  if (entries.length > maxEntries) {
    failures.push(
      `${displayPath(directory)} has ${entries.length} direct entries (maximum ${maxEntries})`,
    );
  }

  for (const entry of entries) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      inspectTree(path);
      continue;
    }
    if (!entry.isFile() || lineExemptFiles.has(entry.name)) {
      continue;
    }
    if (binaryExtensions.has(extname(entry.name).toLowerCase())) {
      continue;
    }
    const content = readFileSync(path, "utf8");
    const lines = content.length === 0 ? 0 : content.split(/\r?\n/u).length;
    if (lines > maxLines) {
      failures.push(
        `${displayPath(path)} has ${lines} lines (maximum ${maxLines})`,
      );
    }
  }
}

function rustSourceFiles(directory) {
  const files = [];
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      files.push(...rustSourceFiles(path));
    } else if (entry.isFile() && entry.name.endsWith(".rs")) {
      files.push(path);
    }
  }
  return files;
}

function inspectRustTests() {
  const cratesRoot = join(root, "packages", "crates");
  if (!statSync(cratesRoot, { throwIfNoEntry: false })?.isDirectory()) {
    return;
  }
  for (const crate of readdirSync(cratesRoot, { withFileTypes: true })) {
    const source = join(cratesRoot, crate.name, "src");
    if (!crate.isDirectory() || !statSync(source, { throwIfNoEntry: false })) {
      continue;
    }
    for (const path of rustSourceFiles(source)) {
      const content = readFileSync(path, "utf8");
      if (/#\s*\[\s*(?:cfg\s*\(\s*test\s*\)|test)\s*\]/u.test(content)) {
        failures.push(`${displayPath(path)} contains an embedded test`);
      }
    }
  }
}

function inspectRuntimeAllocation() {
  const runtimeCrates = ["hearthline-model", "hearthline-engine"];
  const forbidden = [
    [/\bextern\s+crate\s+alloc\b/u, "the alloc crate"],
    [/\bstd::/u, "the standard library"],
    [/\b(?:Box|Rc|Arc|Vec|VecDeque|String|BTreeMap|BTreeSet|HashMap|HashSet)\b/u, "a heap-backed type"],
    [/\b(?:format|vec)!\s*\(/u, "an allocating macro"],
    [/\.(?:boxed|collect|to_owned|to_string)\s*\(/u, "an allocating conversion"],
  ];

  for (const crate of runtimeCrates) {
    const source = join(root, "packages", "crates", crate, "src");
    const crateRoot = join(source, "lib.rs");
    if (!statSync(source, { throwIfNoEntry: false })?.isDirectory()) {
      continue;
    }
    if (!readFileSync(crateRoot, "utf8").includes("#![no_std]")) {
      failures.push(
        `${displayPath(crateRoot)} must compile with #![no_std]`,
      );
    }
    for (const path of rustSourceFiles(source)) {
      const content = readFileSync(path, "utf8");
      const inspected = content
        .replace(/\buse\s+heapless(?:::[^;]+)?;/gu, "")
        .replace(/\bheapless::(?:String|Vec)\b/gu, "FixedCapacity");
      for (const [pattern, description] of forbidden) {
        if (pattern.test(inspected)) {
          failures.push(
            `${displayPath(path)} uses ${description} in allocation-free runtime code`,
          );
        }
      }
    }
  }
}

function inspectRequiredSuites() {
  const requiredFiles = [
    "packages/fuzz/fuzz_targets/identifiers.rs",
    "packages/fuzz/fuzz_targets/appliance_yaml.rs",
    "packages/crates/hearthline-engine/benches/runtime.rs",
  ];
  for (const path of requiredFiles) {
    const absolute = join(root, path);
    if (!statSync(absolute, { throwIfNoEntry: false })?.isFile()) {
      failures.push(`${path} is required`);
    }
  }

  for (const crate of [
    "hearthline-model",
    "hearthline-engine",
    "hearthline-config",
  ]) {
    const tests = join(root, "packages", "crates", crate, "tests");
    const present =
      statSync(tests, { throwIfNoEntry: false })?.isDirectory() &&
      readdirSync(tests).some((entry) => entry.endsWith(".rs"));
    if (!present) {
      failures.push(`${displayPath(tests)} must contain an integration test`);
    }
  }
}

function inspectWorkflow() {
  const workflow = join(root, ".github", "workflows", "ci.yml");
  if (!statSync(workflow, { throwIfNoEntry: false })?.isFile()) {
    failures.push(".github/workflows/ci.yml is required");
    return;
  }
  const content = readFileSync(workflow, "utf8");
  for (const command of [
    "repository-policy.mjs",
    "check-version.mjs",
    "-p hearthline-model",
    "-p hearthline-engine",
    "cargo test",
    "config-validate",
    "config-generate",
    "git diff --exit-code",
    "packages/fuzz/Cargo.toml --all --check",
    "packages/fuzz/Cargo.toml --all-targets",
    "fuzz run identifiers",
    "fuzz run appliance_yaml",
    "cargo bench",
    "npm run check",
    "npm run build",
  ]) {
    if (!content.includes(command)) {
      failures.push(`CI workflow does not run '${command}'`);
    }
  }
}

inspectTree(root);
inspectRustTests();
inspectRuntimeAllocation();
inspectRequiredSuites();
inspectWorkflow();

if (failures.length > 0) {
  console.error("Repository policy violations:");
  for (const failure of failures) {
    console.error(`- ${failure}`);
  }
  process.exitCode = 1;
} else {
  const revision = execFileSync("git", ["rev-parse", "--short", "HEAD"], {
    cwd: root,
    encoding: "utf8",
  }).trim();
  console.log(`Repository policy passed at ${revision}.`);
}
