import { parentPort, workerData } from "node:worker_threads";
import { loadEngine, loadSourceChecker } from "../wasm";
import type { CheckProgressEvent, CheckReport } from "../wasm";
import { walkDirectory } from "../walk";

export type CheckWorkerMessage =
  | { type: "progress"; event: CheckProgressEvent }
  | { type: "report"; report: CheckReport };

// Keep directory reads, compiler loading, and synchronous WASM/TypeScript work
// off the CLI thread so terminal progress continues to animate during checks.
const files = walkDirectory(workerData.input as string);
const progress = (event: CheckProgressEvent) => parentPort!.postMessage({ type: "progress", event } satisfies CheckWorkerMessage);
progress({ type: "loading_checker" });
const report = loadSourceChecker().checkSource(files, loadEngine(), { onProgress: progress });
parentPort!.postMessage({ type: "report", report } satisfies CheckWorkerMessage);
