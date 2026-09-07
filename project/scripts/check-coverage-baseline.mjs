import { execFileSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const baseRevision = process.argv[2];
if (!baseRevision || /^0+$/u.test(baseRevision)) {
  console.log("Coverage baseline comparison skipped: no base revision is available.");
  process.exit(0);
}

const root = resolve(import.meta.dirname, "../..");
const baselinePath = "project/standards/coverage-baseline.json";
const current = JSON.parse(readFileSync(resolve(root, baselinePath), "utf8"));
try {
  execFileSync("git", ["rev-parse", "--verify", `${baseRevision}^{commit}`], {
    cwd: root,
    stdio: "ignore",
  });
} catch (error) {
  throw new Error(`coverage base revision ${baseRevision} is unavailable`, { cause: error });
}

try {
  execFileSync("git", ["cat-file", "-e", `${baseRevision}:${baselinePath}`], {
    cwd: root,
    stdio: "ignore",
  });
} catch {
  console.log(`Coverage baseline is new relative to ${baseRevision}; no prior values can decrease.`);
  process.exit(0);
}

let previous;
try {
  previous = JSON.parse(
    execFileSync("git", ["show", `${baseRevision}:${baselinePath}`], {
      cwd: root,
      encoding: "utf8",
    }),
  );
} catch (error) {
  throw new Error(`cannot read coverage baseline from ${baseRevision}`, { cause: error });
}

const failures = [];
for (const scope of ["deterministic", "critical", "host", "web"]) {
  for (const metric of ["lines", "branches"]) {
    const before = previous[scope]?.[metric];
    const after = current[scope]?.[metric];
    if (typeof before !== "number" || typeof after !== "number") {
      failures.push(`${scope}.${metric} must be numeric in both baselines`);
    } else if (after + 0.0001 < before) {
      failures.push(`${scope}.${metric} decreased from ${before}% to ${after}%`);
    }
  }
}

if (failures.length > 0) {
  throw new Error(`coverage baseline is not monotonic:\n- ${failures.join("\n- ")}`);
}
console.log(`Coverage baseline does not decrease relative to ${baseRevision}.`);
