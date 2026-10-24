import { loadEngine } from "../wasm";
import { walkDirectory } from "../walk";
import { checkTypeScript } from "../check-typescript";

export function cmdCheck(input: string, format: "text" | "json"): void {
  const files = walkDirectory(input);
  const engine = loadEngine();
  const report = engine.check_aix_from_source(files);
  report.diagnostics.push(...checkTypeScript(input, engine.collect_check_scripts(files)));
  report.diagnostics.sort((a, b) => Buffer.compare(Buffer.from(a.path), Buffer.from(b.path))
    || a.line - b.line || a.column - b.column || Buffer.compare(Buffer.from(a.code), Buffer.from(b.code)));
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
