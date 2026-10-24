import path from "node:path";
import { Worker } from "node:worker_threads";
import ora from "ora";
import type { CheckProgressEvent, CheckReport } from "../wasm";
import type { CheckWorkerMessage } from "./check-worker";

export async function cmdCheck(input: string, format: "text" | "json"): Promise<void> {
  const spinner = format === "text" && process.stderr.isTTY
    ? ora({ text: "Scanning source files", stream: process.stderr }).start()
    : undefined;
  const status = (text: string) => {
    if (format === "json") return;
    if (spinner) spinner.text = text;
    else process.stderr.write(`${text}\n`);
  };
  if (!spinner) status("Scanning source files");
  let report: CheckReport;
  try {
    report = await new Promise<CheckReport>((resolve, reject) => {
      const worker = new Worker(path.join(__dirname, "check-worker.js"), { workerData: { input } });
      let result: CheckReport | undefined;
      worker.on("message", (message: CheckWorkerMessage) => {
        if (message.type === "report") result = message.report;
        else status(formatProgress(message.event));
      });
      worker.once("error", reject);
      worker.once("exit", (code) => {
        if (code === 0 && result) resolve(result);
        else reject(new Error(`Check worker exited before returning a report (status ${code})`));
      });
    });
  } finally {
    spinner?.stop();
  }
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

function formatProgress(event: CheckProgressEvent): string {
  switch (event.type) {
    case "loading_checker": return "Loading WASM engine and TypeScript checker";
    case "checking_sources": return `Checking source structure, permissions, and JSON Schemas (${event.fileCount} files)`;
    case "configuring_typescript": return "Reading TypeScript project configuration";
    case "resolving_typescript_project": return `Resolving TypeScript project (${event.rootCount} roots)`;
    case "checking_typescript": return `Checking TypeScript types (${event.rootCount} project roots)`;
    case "collecting_diagnostics": return "Collecting diagnostics";
  }
}
