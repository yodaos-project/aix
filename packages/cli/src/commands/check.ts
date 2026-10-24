import { loadEngine, loadSourceChecker } from "../wasm";
import { walkDirectory } from "../walk";

export function cmdCheck(input: string, format: "text" | "json"): void {
  const report = loadSourceChecker().checkSource(walkDirectory(input), loadEngine());
  if (format === "json") {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    for (const diagnostic of report.diagnostics) {
      process.stdout.write(`${diagnostic.path}:${diagnostic.line}:${diagnostic.column}: ${diagnostic.severity} [${diagnostic.code}] ${diagnostic.message}\n`);
    }
    process.stdout.write(`${report.diagnostics.length} diagnostic(s)\n`);
  }
  if (report.diagnostics.some((diagnostic) => diagnostic.severity === "error")) {
    process.exitCode = 1;
  }
}
