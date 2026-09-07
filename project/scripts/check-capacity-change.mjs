import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseRevision = process.argv[2];
if (!baseRevision || /^0+$/u.test(baseRevision)) {
  console.log("Capacity change comparison skipped: no base revision is available.");
  process.exit(0);
}

const root = resolve(import.meta.dirname, "../..");
const policyPath = "project/standards/engine-capacities.json";
const lockPath = "project/config/model.lock.json";

verifyRevision(baseRevision);
const current = JSON.parse(readFileSync(resolve(root, policyPath), "utf8"));
const previous = readAtRevision(baseRevision, policyPath);
if (previous && stable(previous.capacities) === stable(current.capacities)) {
  console.log("Engine structural capacities are unchanged.");
  process.exit(0);
}

const changed = new Set(
  [
    execFileSync("git", ["diff", "--name-only", baseRevision, "--"], {
      cwd: root,
      encoding: "utf8",
    }),
    execFileSync("git", ["ls-files", "--others", "--exclude-standard"], {
      cwd: root,
      encoding: "utf8",
    }),
  ]
    .flatMap((source) => source.split(/\r?\n/u))
    .filter(Boolean),
);
const failures = [];
const decision = requiredPath(current, "decision");
const benchmark = requiredPath(current, "benchmark");
const tests = Array.isArray(current.tests) ? current.tests : [];

requireChanged(policyPath, "capacity registry");
requireChanged(decision, "capacity ADR");
requireChanged(benchmark, "capacity benchmark");
requireChanged(lockPath, "model lock");
if (!tests.some((path) => changed.has(path))) {
  failures.push("no registered saturation/recovery test changed");
}

const lock = JSON.parse(readFileSync(resolve(root, lockPath), "utf8"));
const decisionName = decision.split("/").at(-1);
if (typeof lock.update_reason !== "string" || !lock.update_reason.includes(decisionName)) {
  failures.push(`model lock update_reason must reference ${decisionName}`);
}

if (failures.length > 0) {
  throw new Error(
    `structural capacity changes require reviewed evidence:\n- ${failures.join("\n- ")}`,
  );
}
console.log(`Structural capacity changes are backed by ${decision}.`);

function requireChanged(path, label) {
  if (!changed.has(path)) failures.push(`${label} ${path} did not change`);
}

function requiredPath(value, field) {
  const path = value[field];
  if (typeof path !== "string" || path.length === 0) {
    throw new Error(`capacity policy omits ${field}`);
  }
  return path;
}

function readAtRevision(revision, path) {
  try {
    return JSON.parse(
      execFileSync("git", ["show", `${revision}:${path}`], {
        cwd: root,
        encoding: "utf8",
        stdio: ["ignore", "pipe", "ignore"],
      }),
    );
  } catch {
    return null;
  }
}

function verifyRevision(revision) {
  try {
    execFileSync("git", ["rev-parse", "--verify", `${revision}^{commit}`], {
      cwd: root,
      stdio: "ignore",
    });
  } catch (error) {
    throw new Error(`capacity base revision ${revision} is unavailable`, { cause: error });
  }
}

function stable(value) {
  return JSON.stringify(value, Object.keys(value ?? {}).sort());
}
