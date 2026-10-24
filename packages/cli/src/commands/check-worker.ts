import { parentPort, workerData } from "node:worker_threads";
import { loadEngine, loadSourceChecker } from "../wasm";
import type { CheckProgressEvent, CheckReport } from "../wasm";
import { walkDirectory } from "../walk";
import { loadTypeEnvironment, readTypeSelection } from "../services/type-environment";

export type CheckWorkerMessage =
  | { type: "type_environment"; version: string }
  | { type: "progress"; event: CheckProgressEvent }
  | { type: "report"; report: CheckReport };

// Keep directory reads, compiler loading, and synchronous WASM/TypeScript work
// off the CLI thread so terminal progress continues to animate during checks.
const progress = (event: CheckProgressEvent) => parentPort!.postMessage({ type: "progress", event } satisfies CheckWorkerMessage);
async function main(): Promise<void> {
  const files = walkDirectory(workerData.input as string);
  // A project dependency is a version choice. Global selection is only a fallback.
  const installed = files.some((file) => file.path.startsWith("node_modules/@yodaos-pkg/ink-env/"));
  const version = workerData.typesVersion as string | undefined ?? (installed ? undefined : readTypeSelection());
  let typeFiles;
  if (version) {
    parentPort!.postMessage({ type: "type_environment", version } satisfies CheckWorkerMessage);
    typeFiles = (await loadTypeEnvironment(version)).files;
  }
  progress({ type: "loading_checker" });
  const report = loadSourceChecker().checkSource(files, loadEngine(), { onProgress: progress, typeFiles });
  parentPort!.postMessage({ type: "report", report } satisfies CheckWorkerMessage);
}
void main();
