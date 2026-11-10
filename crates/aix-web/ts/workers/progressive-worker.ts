/**
 * Runs one staged AIX pack session in a browser worker. Moving scanning,
 * media remuxing, and packaging off the main thread keeps the page responsive.
 *
 * `open` starts the WASM session and reports candidates. `prepare` returns the
 * upload plan and transfers its media fragments to the SDK. The caller's
 * `onProgressiveAsset` callback handles uploads and returns a delivery decision
 * with a receipt for remote assets. `finalize` passes those decisions to WASM
 * to validate receipts and build the .aix archive.
 */
import init, { ProgressivePackWasm } from '../../dist/pkg/aix_web.js';
import type {
  AixInputFile,
  OptimizeOptions,
  PackProgressEvent,
  ProgressiveAssetDecision,
  ProgressiveAssetPlan,
} from '../index.js';

type Request =
  | { type: 'open'; files: AixInputFile[]; fromSource: boolean }
  | { type: 'prepare'; id: string }
  | {
      type: 'finalize';
      decisions: Record<string, ProgressiveAssetDecision>;
      buildId: string;
      engine?: string;
      optimize?: OptimizeOptions;
      agentId?: string;
    };

interface WorkerPort {
  onmessage: ((event: MessageEvent<Request>) => void) | null;
  postMessage(message: unknown, transfer?: Transferable[]): void;
}

const port = globalThis as unknown as WorkerPort;
let session: ProgressivePackWasm | undefined;

port.onmessage = async ({ data }) => {
  try {
    switch (data.type) {
      case 'open': {
        await init();
        session = new ProgressivePackWasm(data.files, data.fromSource);
        port.postMessage({ result: session.candidates() });
        break;
      }
      case 'prepare': {
        if (!session) throw new Error('Progressive Asset session is closed');
        const plan = session.prepare(data.id) as ProgressiveAssetPlan;
        port.postMessage({ result: plan }, plan.remoteObjects.map((object) => object.bytes.buffer as ArrayBuffer));
        break;
      }
      case 'finalize': {
        if (!session) throw new Error('Progressive Asset session is closed');
        const packed = session.finalize(data.decisions, data.buildId, data.engine, data.optimize, data.agentId,
          (progress: PackProgressEvent) => port.postMessage({ progress }));
        try {
          const bytes = packed.data;
          port.postMessage({
            result: {
              data: bytes,
              report: packed.report,
              warnings: packed.warnings,
              progressiveAssets: packed.progressiveAssets,
            },
          }, [bytes.buffer]);
        } finally {
          packed.free();
          session.free();
          session = undefined;
        }
        break;
      }
    }
  } catch (error) {
    port.postMessage({ error: String(error) });
  }
};
