import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const reportPath = process.argv[2];
if (!reportPath) {
  throw new Error("usage: node check-web-coverage.mjs <coverage-summary.json>");
}

const root = resolve(import.meta.dirname, "../..");
const report = JSON.parse(readFileSync(resolve(reportPath), "utf8"));
const baseline = JSON.parse(
  readFileSync(resolve(root, "project/standards/coverage-baseline.json"), "utf8"),
).web;
const floors = { lines: 75, branches: 70 };
const failures = [];

for (const metric of Object.keys(floors)) {
  const actual = report.total?.[metric]?.pct;
  if (typeof actual !== "number") {
    failures.push(`web ${metric} coverage is missing`);
    continue;
  }
  if (actual + 0.0001 < floors[metric]) {
    failures.push(`web ${metric} coverage ${actual}% is below ${floors[metric]}%`);
  }
  if (actual + 0.5 < baseline[metric]) {
    failures.push(
      `web ${metric} coverage ${actual}% regressed more than 0.5 points from ${baseline[metric]}%`,
    );
  }
  console.log(`web ${metric}: ${actual.toFixed(2)}%`);
}

if (failures.length > 0) {
  throw new Error(`web coverage gates failed:\n- ${failures.join("\n- ")}`);
}
