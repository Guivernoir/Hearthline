import { execFileSync } from "node:child_process";
import { readFileSync, readdirSync, statSync } from "node:fs";
import { extname, join, relative, resolve } from "node:path";

const root = resolve(import.meta.dirname, "../..");
const maxSourceLines = 750;
const maxTestLines = 1400;
const maxEntries = 12;
const ignoredDirectories = new Set([
  ".git",
  ".svelte-kit",
  "artifacts",
  "build",
  "corpus",
  "regression-corpus",
  "dist",
  "node_modules",
  "target",
]);
const lineExemptFiles = new Set([
  "Cargo.lock",
  "model.lock.json",
  "openapi.json",
  "package-lock.json",
]);
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
    const repositoryPath = displayPath(path);
    const isTestSource = repositoryPath.split("/").includes("tests");
    const maxLines = isTestSource ? maxTestLines : maxSourceLines;
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

function inspectHmiOwnership() {
  const state = join(
    root,
    "packages",
    "crates",
    "hearthline-sim",
    "src",
    "hmi",
    "state",
    "mod.rs",
  );
  const store = join(
    root,
    "packages",
    "crates",
    "hearthline-sim",
    "src",
    "hmi",
    "state",
    "store.rs",
  );
  if (!statSync(state, { throwIfNoEntry: false })?.isFile()
    || !statSync(store, { throwIfNoEntry: false })?.isFile()) {
    return;
  }
  const stateSource = readFileSync(state, "utf8");
  const storeSource = readFileSync(store, "utf8");
  for (const [pattern, message] of [
    [/sync_shared_from/u, "copies shared plant state into HMI sessions"],
    [/merge_shared_from/u, "copies HMI state back into shared plant state"],
    [/#\[derive\([^\]]*Clone[^\]]*\)\]\s*pub struct PlantControlRuntime/u,
      "allows complete plant control runtimes to be cloned"],
    [/#\[derive\([^\]]*Clone[^\]]*\)\]\s*pub\(super\) struct ControllerRuntime/u,
      "allows authoritative HMI controller runtimes to be cloned"],
  ]) {
    if (pattern.test(`${stateSource}\n${storeSource}`)) {
      failures.push(`HMI ownership ${message}`);
    }
  }
  for (const required of ["HmiCellRuntime", "HmiOperatorContext", "BoundPlantControlRuntime", "impl Drop"] ) {
    if (!storeSource.includes(required)) {
      failures.push(`HMI ownership store does not contain '${required}'`);
    }
  }
}

function inspectControlPhysicsBoundary() {
  const forming = join(
    root,
    "packages",
    "crates",
    "hearthline-engine",
    "src",
    "industrial",
    "process",
    "forming",
  );
  const mould = join(
    root,
    "packages",
    "crates",
    "hearthline-sim",
    "src",
    "hmi",
    "state",
    "mould.rs",
  );
  const contract = join(forming, "control.rs");
  if (!statSync(contract, { throwIfNoEntry: false })?.isFile()
    || !statSync(mould, { throwIfNoEntry: false })?.isFile()) {
    failures.push("IEC/Rust Forming control contract is missing");
    return;
  }
  const contractSource = readFileSync(contract, "utf8");
  const mouldSource = readFileSync(mould, "utf8");
  const orchestrationSource = mouldSource.slice(
    mouldSource.indexOf("pub(super) fn tick"),
    mouldSource.indexOf("pub(in crate::hmi) fn set_fault"),
  );
  for (const required of [
    "FormingControlState",
    "FormingPhysicsInputs",
    "FormingPhysicsFeedback",
  ]) {
    if (!contractSource.includes(required)) {
      failures.push(`IEC/Rust Forming contract does not define '${required}'`);
    }
  }
  for (const required of ["advance_controlled", "scan_with_physics_feedback"]) {
    if (!mouldSource.includes(required)) {
      failures.push(`Forming orchestration does not use '${required}'`);
    }
  }
  for (const [pattern, message] of [
    [/\.tick_controlled\s*\(/u, "calls Rust physics without the typed input contract"],
    [/phase_duration_ms\s*\(/u, "reads Rust timing directly from IEC orchestration"],
    [/synchronize_control_state/u, "uses the obsolete untyped control-state bridge"],
  ]) {
    if (pattern.test(orchestrationSource)) {
      failures.push(`Forming orchestration ${message}`);
    }
  }
}

function inspectRuntimeCapacities() {
  const engine = join(
    root,
    "packages",
    "crates",
    "hearthline-engine",
    "src",
  );
  const ledgerPath = join(engine, "capacity.rs");
  if (!statSync(ledgerPath, { throwIfNoEntry: false })?.isFile()) {
    failures.push("allocator-free runtime capacity ledger is missing");
    return;
  }

  const ledger = readFileSync(ledgerPath, "utf8");
  const declarations = new Set(
    [...ledger.matchAll(
      /^(?:pub(?:\(crate\))?\s+)?const\s+([A-Z][A-Z0-9_]*_CAPACITY):\s*usize\s*=/gmu,
    )].map((match) => match[1]),
  );
  const normalizedLedger = ledger.replace(/\s+/gu, " ");
  const used = new Set();
  const logicalLimits = new Set();

  for (const path of rustSourceFiles(engine)) {
    if (path === ledgerPath) {
      continue;
    }
    const content = readFileSync(path, "utf8");
    if (/^(?:pub(?:\(crate\))?\s+)?const\s+[A-Z][A-Z0-9_]*_CAPACITY:\s*usize\s*=/gmu.test(content)) {
      failures.push(`${displayPath(path)} declares capacity outside the central ledger`);
    }
    for (const [index, line] of content.split(/\r?\n/u).entries()) {
      if (!line.includes("FixedList<") && !line.includes("Deque<")) {
        continue;
      }
      const match = line.match(/,\s*([A-Z][A-Z0-9_]*)\s*>/u);
      if (!match) {
        failures.push(
          `${displayPath(path)}:${index + 1} has an unreadable bounded collection capacity`,
        );
        continue;
      }
      const capacity = match[1];
      const genericCollection =
        (capacity === "N" && path.endsWith("runtime/storage.rs"))
        || (capacity === "CAPACITY" && path.endsWith("industrial/historian.rs"));
      if (genericCollection) {
        continue;
      }
      if (!declarations.has(capacity)) {
        failures.push(
          `${displayPath(path)}:${index + 1} uses unregistered capacity '${capacity}'`,
        );
      } else {
        used.add(capacity);
      }
    }
    for (const capacity of logicalLimits) {
      if (new RegExp(`\\b${capacity}\\b`, "u").test(content)) {
        used.add(capacity);
      }
    }
  }

  for (const capacity of declarations) {
    if (!used.has(capacity)) {
      failures.push(`runtime capacity '${capacity}' has no bounded collection`);
    }
    const budget = new RegExp(
      `CapacityBudget::[a-z_]+\\([^)]*\\b${capacity}\\b`,
      "u",
    );
    if (!budget.test(normalizedLedger)) {
      failures.push(`runtime capacity '${capacity}' has no headroom budget`);
    }
  }
}

function inspectProcessAuthority() {
  const topology = join(root, "project", "config", "ot", "process", "architecture.yaml");
  const generated = join(root, "packages", "web", "src", "generated", "process-view.json");
  const model = join(root, "packages", "web", "src", "lib", "process", "process-model.ts");
  const connections = join(
    root,
    "packages",
    "web",
    "src",
    "lib",
    "process",
    "canvas",
    "ProcessConnections.svelte",
  );
  for (const path of [topology, generated, model, connections]) {
    if (!statSync(path, { throwIfNoEntry: false })?.isFile()) {
      failures.push(`${displayPath(path)} is required for generated process topology`);
      return;
    }
  }
  const generatedSource = readFileSync(generated, "utf8");
  const modelSource = readFileSync(model, "utf8");
  const connectionSource = readFileSync(connections, "utf8");
  if (generatedSource.includes('"generationStatus":"bootstrap"')
    || modelSource.includes('"bootstrap"')) {
    failures.push("frontend process topology still accepts bootstrap authority");
  }
  for (const required of ["processView.networkEdges", "processView.materialFlow", "edgePath(edge)"]) {
    if (!connectionSource.includes(required)) {
      failures.push(`process connections do not derive '${required}' from generated topology`);
    }
  }
}

function inspectTelemetryCapacity() {
  const model = join(root, "packages", "crates", "hearthline-model", "src", "application.rs");
  const builder = join(
    root,
    "packages",
    "crates",
    "hearthline-sim",
    "src",
    "hmi",
    "builder",
    "support.rs",
  );
  if (!statSync(model, { throwIfNoEntry: false })?.isFile()
    || !statSync(builder, { throwIfNoEntry: false })?.isFile()) {
    failures.push("bounded telemetry payload contract is missing");
    return;
  }
  const modelSource = readFileSync(model, "utf8");
  const builderSource = readFileSync(builder, "utf8");
  for (const required of [
    "TELEMETRY_PAYLOAD_CAPACITY",
    "TELEMETRY_NOMINAL_PAYLOAD_BYTES",
    "TELEMETRY_NOMINAL_PAYLOAD_BYTES * 4 <= TELEMETRY_PAYLOAD_CAPACITY * 3",
  ]) {
    if (!modelSource.includes(required)) {
      failures.push(`telemetry model does not enforce '${required}'`);
    }
  }
  for (const required of ["telemetry_precision", "TELEMETRY_NOMINAL_PAYLOAD_BYTES", "v: 1"]) {
    if (!builderSource.includes(required)) {
      failures.push(`Forming telemetry builder does not enforce '${required}'`);
    }
  }
}

function inspectRequiredSuites() {
  const requiredFiles = [
    "packages/fuzz/fuzz_targets/identifiers.rs",
    "packages/fuzz/fuzz_targets/appliance_yaml.rs",
    "packages/fuzz/fuzz_targets/connection_yaml.rs",
    "packages/fuzz/fuzz_targets/blueprint_yaml.rs",
    "packages/fuzz/fuzz_targets/model_compile.rs",
    "packages/fuzz/fuzz_targets/partition_runtime.rs",
    "packages/fuzz/fuzz_targets/snapshot_replay.rs",
    "packages/fuzz/fuzz_targets/control_program.rs",
    "packages/crates/hearthline-engine/benches/runtime.rs",
    "packages/crates/hearthline-sim/benches/scheduler.rs",
    "project/config/model.lock.json",
    "project/standards/coverage-baseline.json",
    "project/scripts/check-coverage.mjs",
    "project/scripts/check-coverage-baseline.mjs",
    "project/scripts/check-capacity-change.mjs",
    "project/scripts/check-web-coverage.mjs",
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
    "hearthline-project",
    "hearthline-sim",
    "hearthline-operator",
    "hearthline-cli",
    "hearthline-api",
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
  const nightly = join(root, ".github", "workflows", "nightly.yml");
  if (!statSync(workflow, { throwIfNoEntry: false })?.isFile()
    || !statSync(nightly, { throwIfNoEntry: false })?.isFile()) {
    failures.push("PR and nightly GitHub Actions workflows are required");
    return;
  }
  const content = readFileSync(workflow, "utf8");
  for (const command of [
    "repository-policy.mjs",
    "check-version.mjs",
    "check-coverage-baseline.mjs",
    "check-capacity-change.mjs",
    "cargo xtask verify",
    "cargo xtask contracts --check",
    "cargo test",
    "model validate",
    "model compile --locked",
    "capacity report --format json",
    "git diff --exit-code",
    "packages/fuzz/Cargo.toml --all --check",
    "packages/fuzz/Cargo.toml --all-targets",
    "target: identifiers",
    "target: appliance_yaml",
    "target: partition_runtime",
    "fuzz run ${{ matrix.target }}",
    "npm run check",
    "npm run test:unit",
    "npm run test:e2e",
    "npm run build",
    "ubuntu-24.04-arm",
    "windows-2025",
    "macos-14",
  ]) {
    if (!content.includes(command)) {
      failures.push(`CI workflow does not run '${command}'`);
    }
  }
  const nightlyContent = readFileSync(nightly, "utf8");
  for (const command of [
    "llvm-cov --branch",
    "check-coverage.mjs",
    "cargo mutants",
    "miri test",
    "cargo audit",
    "cargo cyclonedx",
    "--test acceptance --release",
    "cargo bench",
    "HEARTHLINE_ALL_BROWSERS",
    "retention-days: 90",
  ]) {
    if (!nightlyContent.includes(command)) {
      failures.push(`nightly workflow does not run '${command}'`);
    }
  }
  for (const command of [
    "cargo mutants --manifest-path packages/Cargo.toml --copy-target false",
    "test -n \"$mutants\"",
    "--file crates/hearthline-engine/src/industrial/process/safety.rs",
    "--file crates/hearthline-sim/src/scheduler.rs",
    "--file crates/hearthline-project/src/blueprint.rs",
    "miri test --manifest-path packages/Cargo.toml -p hearthline-model --test domain --test quantity",
    "miri test --manifest-path packages/Cargo.toml -p hearthline-engine --lib --test link_appliance",
    "--bench runtime -- --noplot",
    "--bench scheduler -- --noplot",
    "cargo cyclonedx --manifest-path packages/Cargo.toml --format json",
    "packages/**/*.cdx.json",
    "packages/mutants.out",
  ]) {
    if (!nightlyContent.includes(command)) {
      failures.push(`nightly workflow is missing hardened command '${command}'`);
    }
  }
  for (const obsolete of [
    "--file packages/crates/",
    "cargo cyclonedx --manifest-path packages/Cargo.toml --workspace",
    "-p hearthline-engine -p hearthline-sim -- --noplot",
    "path: mutants.out",
  ]) {
    if (nightlyContent.includes(obsolete)) {
      failures.push(`nightly workflow contains obsolete command '${obsolete}'`);
    }
  }
  for (const [path, source] of [[workflow, content], [nightly, nightlyContent]]) {
    for (const match of source.matchAll(/^\s*- uses:\s*([^\s]+)$/gmu)) {
      if (!/@[0-9a-f]{40}$/u.test(match[1])) {
        failures.push(`${displayPath(path)} has an unpinned action '${match[1]}'`);
      }
    }
  }
}

function inspectToolchainPins() {
  const expected = {
    rust: "1.98.1",
    node: "26.8.1",
    npm: "12.0.2",
    svelteKit: "2.70.3",
  };
  const rustToolchain = readFileSync(join(root, "rust-toolchain.toml"), "utf8");
  const nodeVersion = readFileSync(join(root, ".nvmrc"), "utf8").trim();
  const webPackage = JSON.parse(
    readFileSync(join(root, "packages", "web", "package.json"), "utf8"),
  );
  const webLock = JSON.parse(
    readFileSync(join(root, "packages", "web", "package-lock.json"), "utf8"),
  );
  const lockedRoot = webLock.packages?.[""] ?? {};

  if (!rustToolchain.includes(`channel = "${expected.rust}"`)) {
    failures.push(`rust-toolchain.toml must pin Rust ${expected.rust}`);
  }
  if (nodeVersion !== expected.node || webPackage.engines?.node !== expected.node
    || lockedRoot.engines?.node !== expected.node) {
    failures.push(`.nvmrc and npm metadata must pin Node ${expected.node}`);
  }
  if (webPackage.engines?.npm !== expected.npm
    || lockedRoot.engines?.npm !== expected.npm
    || webPackage.packageManager !== `npm@${expected.npm}`) {
    failures.push(`npm metadata must pin npm ${expected.npm}`);
  }
  if (webPackage.devDependencies?.["@sveltejs/kit"] !== expected.svelteKit
    || lockedRoot.devDependencies?.["@sveltejs/kit"] !== expected.svelteKit) {
    failures.push(`npm metadata must pin SvelteKit ${expected.svelteKit}`);
  }
  const viteConfig = readFileSync(join(root, "packages", "web", "vite.config.ts"), "utf8");
  const svelteConfig = readFileSync(join(root, "packages", "web", "svelte.config.js"), "utf8");
  if (!viteConfig.includes("@sveltejs/kit/vite") || !viteConfig.includes("sveltekit()")) {
    failures.push("the frontend must use SvelteKit's Vite plugin");
  }
  if (!svelteConfig.includes("@sveltejs/adapter-static")) {
    failures.push("the SvelteKit build must declare its static adapter");
  }
}

inspectTree(root);
inspectRuntimeAllocation();
inspectHmiOwnership();
inspectControlPhysicsBoundary();
inspectRuntimeCapacities();
inspectProcessAuthority();
inspectTelemetryCapacity();
inspectRequiredSuites();
inspectWorkflow();
inspectToolchainPins();

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
