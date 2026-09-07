import { readFileSync } from "node:fs";
import { resolve } from "node:path";

const reportPath = process.argv[2];
if (!reportPath) throw new Error("usage: node check-coverage.mjs <llvm-cov.json>");

const root = resolve(import.meta.dirname, "../..");
const report = JSON.parse(readFileSync(resolve(reportPath), "utf8"));
const baseline = JSON.parse(
  readFileSync(resolve(root, "project/standards/coverage-baseline.json"), "utf8"),
);
const files = report.data?.[0]?.files ?? [];

const groups = [
  { id: "deterministic", pattern: /hearthline-(model|engine|project|sim)\/src\//u, lines: 85, branches: 80 },
  { id: "host", pattern: /hearthline-(config|operator|api|cli)\/src\//u, lines: 80, branches: 75 },
  {
    id: "critical",
    pattern: /(safety\.rs|capacity\.rs|scheduler\.rs|replay\.rs|blueprint(?:\/|\.rs)|forming\/control\.rs)$/u,
    lines: 95,
    branches: 90,
  },
];

const failures = [];
for (const group of groups) {
  const selected = files.filter((file) => group.pattern.test(String(file.filename).replaceAll("\\", "/")));
  if (selected.length === 0) {
    failures.push(`${group.id} coverage matched no files`);
    continue;
  }
  const result = summarize(selected);
  if (result.branchCount === 0) {
    failures.push(`${group.id} coverage contains no instrumented branches`);
  }
  for (const metric of ["lines", "branches"]) {
    const minimum = group[metric];
    const prior = baseline[group.id]?.[metric];
    const actual = result[metric];
    if (actual + 0.0001 < minimum) {
      failures.push(`${group.id} ${metric} coverage ${actual.toFixed(2)}% is below ${minimum}%`);
    }
    if (typeof prior === "number" && actual + 0.5 < prior) {
      failures.push(
        `${group.id} ${metric} coverage ${actual.toFixed(2)}% regressed more than 0.5 points from ${prior}%`,
      );
    }
  }
  console.log(`${group.id}: ${result.lines.toFixed(2)}% lines, ${result.branches.toFixed(2)}% branches`);
}

if (failures.length > 0) {
  throw new Error(`coverage gates failed:\n- ${failures.join("\n- ")}`);
}

function summarize(selected) {
  const totals = { lines: { count: 0, covered: 0 }, branches: { count: 0, covered: 0 } };
  for (const file of selected) {
    for (const metric of Object.keys(totals)) {
      totals[metric].count += file.summary?.[metric]?.count ?? 0;
      totals[metric].covered += file.summary?.[metric]?.covered ?? 0;
    }
  }
  return {
    ...Object.fromEntries(Object.entries(totals).map(([metric, value]) => [
      metric,
      value.count === 0 ? 100 : (value.covered * 100) / value.count,
    ])),
    branchCount: totals.branches.count,
  };
}
