import path from "node:path";
import { Worker } from "node:worker_threads";
import ora from "ora";
import type { CheckProgressEvent, CheckReport, CheckResult } from "../wasm";
import type { CheckWorkerMessage } from "./check-worker";

export async function cmdCheck(input: string, format: "text" | "json", typesVersion?: string): Promise<void> {
  if (format === "text" && process.stderr.isTTY) {
    // Replace the bootstrap status on the same line before Ora takes over.
    process.stderr.cursorTo(0);
    process.stderr.clearLine(0);
  }
  const spinner = format === "text" && process.stderr.isTTY
    ? ora({ text: "Scanning source files", stream: process.stderr }).start()
    : undefined;
  const status = (text: string) => {
    if (format === "json") return;
    if (spinner) spinner.text = text;
    else process.stderr.write(`${text}\n`);
  };
  if (!spinner) status("Scanning source files");
  const checks: CheckResult[] = [];
  let report: CheckReport;
  try {
    report = await new Promise<CheckReport>((resolve, reject) => {
      const worker = new Worker(path.join(__dirname, "check-worker.js"), { workerData: { input, typesVersion } });
      let result: CheckReport | undefined;
      worker.on("message", (message: CheckWorkerMessage) => {
        if (message.type === "report") result = message.report;
        else if (message.type === "type_environment") status(`Loading Ink type declarations (${message.version})`);
        else if (message.event.type === "check_finished") checks.push(message.event.check);
        else status(formatProgress(message.event));
      });
      worker.once("error", reject);
      worker.once("exit", (code) => {
        if (code === 0 && result) resolve(result);
        else reject(new Error(`Check worker exited before returning a report (status ${code})`));
      });
    });
  } catch (error) {
    if (spinner) spinner.fail("Checks failed");
    else status("Checks failed");
    throw error;
  }
  const errors = report.diagnostics.filter((diagnostic) => diagnostic.severity === "error").length;
  if (format === "text") {
    for (const check of checks) {
      const counts = [
        check.errors ? `${check.errors} error${check.errors === 1 ? "" : "s"}` : "",
        check.warnings ? `${check.warnings} warning${check.warnings === 1 ? "" : "s"}` : "",
      ].filter(Boolean).join(", ");
      const detail = check.detail ? ` (${check.status === "skipped" ? "skipped: " : ""}${check.detail})` : "";
      const text = `${check.label}${counts ? `: ${counts}` : ""}${detail}`;
      if (!spinner) status(`[${check.status}] ${text}`);
      else if (check.status === "failed") spinner.fail(text);
      else if (check.status === "warning") spinner.warn(text);
      else if (check.status === "skipped") spinner.stopAndPersist({ symbol: "-", text });
      else spinner.succeed(text);
    }
  }
  if (format === "json") {
    process.stdout.write(`${JSON.stringify(report, null, 2)}\n`);
  } else {
    for (const diagnostic of report.diagnostics) {
      process.stdout.write(`${diagnostic.path}:${diagnostic.line}:${diagnostic.column}: ${diagnostic.severity} [${diagnostic.code}] ${diagnostic.message}\n`);
    }
    process.stdout.write(`${report.diagnostics.length} diagnostic(s)\n`);
  }
  if (errors) {
    process.exitCode = 1;
  }
}

function formatProgress(event: Exclude<CheckProgressEvent, { type: "check_finished" }>): string {
  switch (event.type) {
    case "loading_checker": return "Loading WASM engine and TypeScript checker";
    case "checking_sources": return `Checking source structure, permissions, and JSON Schemas (${event.fileCount} files)`;
    case "configuring_typescript": return "Reading TypeScript project configuration";
    case "resolving_typescript_project": return `Resolving TypeScript project (${event.rootCount} roots)`;
    case "checking_typescript": return `Checking TypeScript types (${event.rootCount} project roots)`;
    case "collecting_diagnostics": return "Collecting diagnostics";
  }
}
