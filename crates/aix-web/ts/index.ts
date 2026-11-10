import initReader, { AixReaderWasm } from '../dist/pkg-reader/aix_web.js';
import type { CheckDiagnostic, CheckOptions, CheckReport } from './check/types.js';

type Tooling = typeof import('../dist/pkg/aix_web.js');
let toolingPromise: Promise<Tooling> | undefined;
let readerPromise: ReturnType<typeof initReader> | undefined;

function initializeReader(): ReturnType<typeof initReader> {
  return readerPromise ??= initReader().catch((error) => {
    readerPromise = undefined;
    throw error;
  });
}

// A shared promise prevents concurrent pack/check calls from instantiating
// separate memories. A failed fetch may be retried by a later SDK call.
function loadTooling(): Promise<Tooling> {
  return toolingPromise ??= import('../dist/pkg/aix_web.js').then(async (module) => {
    await module.default();
    return module;
  }).catch((error) => {
    toolingPromise = undefined;
    throw error;
  });
}

export type { CheckDiagnostic, CheckOptions, CheckProgressEvent, CheckReport, CheckResult } from './check/types.js';

/** A package failed the optional source check in AIX.From(). */
export class AixCheckError extends Error {
  constructor(readonly report: CheckReport) {
    const count = report.diagnostics.filter(diagnostic => diagnostic.severity === 'error').length;
    super(`AIX check failed with ${count} ${count === 1 ? 'error' : 'errors'}`);
    this.name = 'AixCheckError';
  }

  /** All check diagnostics, including warnings. */
  get diagnostics(): CheckDiagnostic[] {
    return this.report.diagnostics;
  }
}

export interface AixInputFile {
  path: string;
  data: Uint8Array;
}

export interface OptimizeOptions {
  level?: 1 | 2 | 3;
  json?: boolean;
  png?: boolean;
  jpeg?: boolean;
}

export interface FileOptimizeReport {
  path: string;
  status: 'optimized' | 'unchanged' | 'skipped';
  original_size: number;
  output_size: number;
  saved_bytes: number;
  converted_to_utf8: boolean;
}

export interface OptimizeReport {
  files: FileOptimizeReport[];
  original_size: number;
  output_size: number;
  saved_bytes: number;
}

export interface PackResult {
  data: Uint8Array;
  report: OptimizeReport;
  warnings: string[];
  progressiveAssets: ProgressiveAssetReport[];
}

export type AssetRef = `aix-asset:${string}`;
export type ProgressiveAssetKind = 'audio' | 'video';

export interface ProgressiveAssetCandidate {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  sourceSha256: string;
  sourceSize: number;
  references: { path: string; line: number; column: number }[];
  format: string;
  dependencies: string[];
  progressiveSupport: {
    supported: boolean;
    profile?: string;
    requiredCapabilities: string[];
    features: string[];
    reason?: string;
  };
}

export interface ProgressiveAssetObject {
  id: string;
  size: number;
  sha256: string;
}

export interface ProgressiveAssetPlan {
  id: string;
  assetVersion: string;
  profile: string;
  localMetadata: Record<string, Omit<ProgressiveAssetObject, 'id'>>;
  layout: {
    init: string;
    tracks: Record<string, { kind: ProgressiveAssetKind; timescale: number }>;
    fragments: { object: string; ranges: Record<string, { start: number; end: number; presentation_start: number; presentation_end: number }> }[];
  };
  remoteObjects: (ProgressiveAssetObject & { bytes: Uint8Array })[];
}

export interface ProgressiveAssetUploadReceipt {
  uploadId: string;
  assetVersion: string;
  objects: (ProgressiveAssetObject & { url: string })[];
}

export type ProgressiveAssetDecision =
  | { delivery: 'embedded' }
  | { delivery: 'progressive'; planId: string; receipt: ProgressiveAssetUploadReceipt };

export interface ProgressiveAssetPackContext {
  signal: AbortSignal;
  prepare(): Promise<ProgressiveAssetPlan>;
}

export interface ProgressiveAssetReport {
  id: string;
  kind: ProgressiveAssetKind;
  sourcePath: string;
  delivery: 'embedded' | 'progressive';
  assetVersion?: string;
  retainedSourceBytes: number;
}

export interface PackOptions {
  onProgressiveAsset?: (
    candidate: ProgressiveAssetCandidate,
    context: ProgressiveAssetPackContext,
  ) => ProgressiveAssetDecision | Promise<ProgressiveAssetDecision>;
  signal?: AbortSignal;
  buildId?: string;
  /** Stable normalized project ID; reuse across builds. */
  agentId?: string;
  engine?: string;
  optimize?: false | OptimizeOptions;
}

export type PackProgressEvent =
  | { type: 'transferring_files_to_wasm' }
  | { type: 'collecting_source_inputs' }
  | { type: 'resolving_engine' }
  | { type: 'preparing_files' }
  | { type: 'file_finished'; report: FileOptimizeReport }
  | { type: 'finalizing_archive' };

export interface PackFromSourceOptions extends PackOptions {
  onProgress?: (event: PackProgressEvent) => void;
}

type PackFromSourceFn = (
  files: AixInputFile[],
  buildId: string,
  engine: string | undefined,
  optimize: false | OptimizeOptions | undefined,
  agentId?: string,
) => { data: Uint8Array; report: unknown; warnings?: string[]; progressiveAssets?: ProgressiveAssetReport[] };

type PackFromSourceWithProgressFn = (
  files: AixInputFile[],
  buildId: string,
  engine: string | undefined,
  optimize: false | OptimizeOptions | undefined,
  progress: (event: PackProgressEvent) => void,
  agentId?: string,
) => { data: Uint8Array; report: unknown; warnings?: string[]; progressiveAssets?: ProgressiveAssetReport[] };

function generateBuildId(): string {
  const cryptoApi = globalThis.crypto;
  if (typeof cryptoApi?.randomUUID === 'function') {
    return cryptoApi.randomUUID();
  }

  const bytes = new Uint8Array(16);
  if (typeof cryptoApi?.getRandomValues === 'function') {
    cryptoApi.getRandomValues(bytes);
  } else {
    const timestamp = Date.now();
    for (let index = 0; index < bytes.length; index += 1) {
      bytes[index] = Math.floor(Math.random() * 256) ^ (timestamp >>> ((index % 6) * 8));
    }
  }
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (value) => value.toString(16).padStart(2, '0')).join('');
  return `${hex.slice(0, 8)}-${hex.slice(8, 12)}-${hex.slice(12, 16)}-${hex.slice(16, 20)}-${hex.slice(20)}`;
}

export interface AixEntry {
  name: string;
  size: number;
  compressed_size: number;
}

/** Inclusive archive budgets. Omitted fields are unlimited for compatibility. */
export interface AixReadLimits {
  /** Maximum central-directory entries, including directories and metadata. */
  maxEntries?: number;
  /** Maximum declared and extracted bytes per entry. */
  maxEntryBytes?: number;
  /** Maximum total declared uncompressed bytes; repeated reads do not consume it. */
  maxTotalBytes?: number;
}

/** JSON returned by the reader, including unvalidated legacy schema values. */
export type JsonValue = null | boolean | number | string | JsonValue[] | JsonObject;

export interface JsonObject {
  [key: string]: JsonValue;
}

/** Schema payloads retain any JSON value; source checking validates the schema. */
export type JsonSchema = JsonValue;

export interface PageConstraint {
  width: number;
  height: number;
}

export interface PageInfo {
  name: string;
  title: string | null;
  description: string | null;
  data_schema: JsonSchema;
  size: PageConstraint;
}

export interface MetadataDiagnostic {
  /** Archive path of the missing, unreadable, or malformed resource. */
  path: string;
  message: string;
}

export interface MetadataResolution {
  pages: PageInfo[];
  tools: Tool[];
  /** One failure per unresolved page, or an app.json configuration failure. */
  diagnostics: MetadataDiagnostic[];
}

export type WidgetPlacement = 'persistent' | 'overlay';

export interface WidgetInfo {
  path: string;
  family: string;
  placement: WidgetPlacement;
  displayName: string | null;
  description: string | null;
}

/** Resolved presentation target returned by the reader. */
export type ToolTarget = '_inline' | '_blank';

export interface FunctionDefinition {
  name: string;
  description: string | null;
  parameters: JsonSchema;
}

export interface Tool {
  /** The reader currently emits "function". */
  type: string;
  target: ToolTarget;
  layout: PageConstraint;
  function: FunctionDefinition;
}

export class AIX {
  private reader: AixReaderWasm;

  private constructor(reader: AixReaderWasm) {
    this.reader = reader;
  }

  /** Run source structure, JS/TS syntax, schema, and permission checks in memory. */
  static async checkFromSource(files: AixInputFile[], options?: CheckOptions): Promise<CheckReport> {
    options?.onProgress?.({ type: 'loading_checker' });
    const [tooling, { checkSource }] = await Promise.all([loadTooling(), import('./check/index.js')]);
    return checkSource(files, tooling, options);
  }

  /** Check a browser directory selection, retaining webkitRelativePath. */
  static async checkFromFiles(files: File[], options?: CheckOptions): Promise<CheckReport> {
    const sources = await Promise.all(files.map(async (file) => ({
      path: resolveWebFilePath(file),
      data: new Uint8Array(await file.arrayBuffer()),
    })));
    const selectedRoot = sources[0]?.path.split('/')[0];
    if (selectedRoot && files.every((file) => file.webkitRelativePath?.startsWith(`${selectedRoot}/`))) {
      for (const source of sources) source.path = source.path.slice(selectedRoot.length + 1);
    }
    return AIX.checkFromSource(sources, options);
  }

  /**
   * Initialize the WASM module and create an AIX instance from the given data.
   * @param data The .aix file content as Uint8Array or File
   * @param limits Caller budgets; set all three when accepting untrusted archives.
   * Values must be non-negative safe integers. Without limits, retains the
   * historical unlimited policy. Input bytes and caller-retained copies are
   * outside these budgets.
   * @param check Run the source checker before returning; defaults to false.
   * Error diagnostics reject with AixCheckError; warnings alone are accepted.
   */
  static async From(data: Uint8Array | File, limits?: AixReadLimits, check = false): Promise<AIX> {
    await initializeReader();
    let buffer: Uint8Array;
    if (data instanceof Uint8Array) {
      buffer = data;
    } else {
      const arrayBuffer = await data.arrayBuffer();
      buffer = new Uint8Array(arrayBuffer);
    }
    const reader = limits === undefined
      ? new AixReaderWasm(buffer)
      : AixReaderWasm.with_limits(buffer, limits);
    try {
      if (check) {
        const files = (reader.list() as AixEntry[])
          .filter(entry => !entry.name.endsWith('/') && entry.name !== 'VERSION'
            && !entry.name.startsWith('META-INF/aix/'))
          .map(entry => ({ path: entry.name, data: reader.read_file(entry.name) }));
        const report = await AIX.checkFromSource(files);
        if (report.diagnostics.some(diagnostic => diagnostic.severity === 'error')) {
          throw new AixCheckError(report);
        }
      }
      return new AIX(reader);
    } catch (error) {
      reader.free();
      throw error;
    }
  }

  /** Lists the Progressive Assets declared in the same normalized inputs as {@link pack}. */
  static async listProgressiveAssets(
    files: AixInputFile[],
    options?: { signal?: AbortSignal },
  ): Promise<ProgressiveAssetCandidate[]> {
    const signal = options?.signal ?? new AbortController().signal;
    signal.throwIfAborted();
    const session = await openPackSession(snapshotFiles(files), false, signal, () => {});
    try {
      signal.throwIfAborted();
      return session.candidates;
    } finally {
      session.close();
    }
  }

  static async pack(
    files: AixInputFile[],
    options?: PackOptions,
  ): Promise<PackResult> {
    if (options?.onProgressiveAsset || options?.signal) {
      return stagedPack(files, options, false);
    }
    const tooling = await loadTooling();
    const buildId = options?.buildId ?? generateBuildId();
    const optimize = options?.optimize === false ? undefined : options?.optimize;
    const result = tooling.pack_aix(files, buildId, options?.engine, optimize, options?.agentId);
    return packResult(result);
  }

  /**
   * Packs an in-memory application source tree.
   *
   * Unlike {@link pack}, this applies source collection semantics before
   * packing: paths are normalized, duplicate paths are rejected, nested
   * `.aixignore` files are honored, and the rule files are omitted.
   */
  static async packFromSource(
    files: AixInputFile[],
    options?: PackFromSourceOptions,
  ): Promise<PackResult> {
    if (options?.onProgressiveAsset || options?.signal) {
      return stagedPack(files, options, true);
    }
    const tooling = await loadTooling();
    const buildId = options?.buildId ?? generateBuildId();
    const optimize = options?.optimize === false ? undefined : options?.optimize;
    const result = options?.onProgress
      ? getPackFromSourceWithProgress(tooling)(
          files,
          buildId,
          options.engine,
          optimize,
          options.onProgress,
          options.agentId,
        )
      : getPackFromSource(tooling)(files, buildId, options?.engine, optimize, options?.agentId);
    return packResult(result);
  }

  static async packFromFiles(files: File[], options?: PackFromSourceOptions): Promise<PackResult> {
    const sourceFiles = await Promise.all(files.map(async (file) => ({
      path: resolveWebFilePath(file),
      data: new Uint8Array(await file.arrayBuffer()),
    })));
    return AIX.packFromSource(sourceFiles, options);
  }

  static async optimize(
    data: Uint8Array | File,
    options?: OptimizeOptions,
  ): Promise<PackResult> {
    const tooling = await loadTooling();
    const buffer = data instanceof Uint8Array
      ? data
      : new Uint8Array(await data.arrayBuffer());
    const result = tooling.optimize_aix(buffer, options);
    return packResult(result);
  }

  /**
   * List all files in the AIX package.
   */
  list(): AixEntry[] {
    return this.reader.list() as AixEntry[];
  }

  /**
   * Read the content of a file from the AIX package.
   * @param name The name of the file
   */
  readFile(name: string): Uint8Array {
    return this.reader.read_file(name);
  }

  /**
   * Get the version metadata from the AIX package.
   */
  getVersion(): string | undefined {
    return this.reader.get_version();
  }

  /** Stable project ID from the manifest; undefined for legacy packages. */
  getAgentId(): string | undefined {
    return this.reader.get_agent_id();
  }

  supportsEngine(currentVersion: string): boolean {
    return this.reader.supports_engine(currentVersion);
  }

  /**
   * Get the title from app.json.
   */
  getTitle(): string | undefined {
    return this.reader.get_title();
  }

  /**
   * Get permission names declared in app.json, in declaration order.
   * Missing declarations return []; invalid configuration throws.
   * This does not query runtime or device authorization.
   */
  getPermissions(): string[] {
    return this.reader.get_permissions() as string[];
  }

  /**
   * Get all pages from app.json and pages/*.json.
   */
  getPages(): PageInfo[] {
    return this.reader.get_pages() as PageInfo[];
  }

  /**
   * Resolve readable page resources and metadata, excluding failed pages/tools.
   * Inspect diagnostics to distinguish a valid empty result from resolution errors.
   * Widget/title fields are independent. This does not run full source checks.
   */
  resolveMetadata(): MetadataResolution {
    return this.reader.resolve_metadata() as MetadataResolution;
  }

  /**
   * Get all widgets from app.json after validating that each .ink entry exists.
   * When locale is provided, merge displayName and description from app.<locale>.json.
   */
  getWidgets(locale?: string): WidgetInfo[] {
    return this.reader.get_widgets(locale) as WidgetInfo[];
  }

  /**
   * Get all tools from app.json and pages/*.json in OpenAI format.
   */
  getTools(): Tool[] {
    return this.reader.get_tools() as Tool[];
  }
}

function resolveWebFilePath(file: File): string {
  const candidate = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
  return candidate && candidate.length > 0 ? candidate : file.name;
}

function getPackFromSource(tooling: Tooling): PackFromSourceFn {
  const packFromSource = tooling.pack_aix_from_source;
  if (!packFromSource) {
    throw new Error('pack_aix_from_source is not available in the current WASM bundle');
  }
  return packFromSource;
}

function getPackFromSourceWithProgress(tooling: Tooling): PackFromSourceWithProgressFn {
  const packFromSourceWithProgress = tooling.pack_aix_from_source_with_progress;
  if (!packFromSourceWithProgress) {
    throw new Error('pack_aix_from_source_with_progress is not available in the current WASM bundle');
  }
  return packFromSourceWithProgress;
}

type NativePackResult = {
  data: Uint8Array;
  report: unknown;
  warnings?: string[];
  progressiveAssets?: ProgressiveAssetReport[];
  free?: () => void;
};

function packResult(result: NativePackResult): PackResult {
  try {
    return {
      data: result.data,
      report: result.report as OptimizeReport,
      warnings: result.warnings ?? [],
      progressiveAssets: result.progressiveAssets ?? [],
    };
  } finally {
    result.free?.();
  }
}

interface PackSession {
  candidates: ProgressiveAssetCandidate[];
  prepare(id: string): Promise<ProgressiveAssetPlan>;
  finalize(
    decisions: Record<string, ProgressiveAssetDecision>,
    buildId: string,
    engine?: string,
    optimize?: OptimizeOptions,
    agentId?: string,
  ): Promise<PackResult>;
  close(): void;
}

async function openPackSession(
  files: AixInputFile[],
  fromSource: boolean,
  signal: AbortSignal,
  progress: (event: PackProgressEvent) => void,
): Promise<PackSession> {
  let worker: Worker | undefined;
  if (typeof Worker !== 'undefined' && typeof document !== 'undefined') {
    try {
      worker = new Worker(new URL('./workers/progressive-worker.js', import.meta.url), { type: 'module' });
    } catch {
      // A host can forbid workers. Use its existing WASM event loop.
    }
  }
  if (!worker) {
    const tooling = await loadTooling();
    signal.throwIfAborted();
    const session = new tooling.ProgressivePackWasm(files, fromSource);
    try {
      return {
        candidates: session.candidates() as ProgressiveAssetCandidate[],
        prepare: async (id) => session.prepare(id) as ProgressiveAssetPlan,
        finalize: async (decisions, buildId, engine, optimize, agentId) =>
          packResult(session.finalize(decisions, buildId, engine, optimize, agentId, progress)),
        close: () => session.free(),
      };
    } catch (error) {
      session.free();
      throw error;
    }
  }

  const port = worker;
  const call = <T>(request: unknown, transfer: Transferable[] = []) =>
    untilAborted(signal, () => new Promise<T>((resolve, reject) => {
      port.onmessage = ({ data }) => {
        if (data.progress) {
          try {
            progress(data.progress);
          } catch (error) {
            reject(error);
            port.terminate();
          }
        } else if (data.error !== undefined) {
          reject(new Error(data.error));
        } else {
          resolve(data.result as T);
        }
      };
      port.onerror = (event) => reject(new Error(event.message || 'Progressive Asset worker failed'));
      port.onmessageerror = () => reject(new Error('Invalid Progressive Asset worker reply'));
      port.postMessage(request, transfer);
    }));
  try {
    const candidates = await call<ProgressiveAssetCandidate[]>({ type: 'open', files, fromSource }, files.map((file) => file.data.buffer as ArrayBuffer));
    return {
      candidates,
      prepare: (id) => call({ type: 'prepare', id }),
      finalize: (decisions, buildId, engine, optimize, agentId) =>
        call({ type: 'finalize', decisions, buildId, engine, optimize, agentId }),
      close: () => port.terminate(),
    };
  } catch (error) {
    port.terminate();
    throw error;
  }
}

function snapshotFiles(files: AixInputFile[]): AixInputFile[] {
  return files.map(({ path, data }) => ({ path, data: Uint8Array.from(data) }));
}

async function stagedPack(
  files: AixInputFile[],
  options: PackFromSourceOptions,
  fromSource: boolean,
): Promise<PackResult> {
  const { onProgress, onProgressiveAsset, engine, agentId } = options;
  const buildId = options.buildId ?? generateBuildId();
  const optimize = options.optimize ? { ...options.optimize } : undefined;
  const signal = options.signal ?? new AbortController().signal;
  signal.throwIfAborted();
  const snapshot = snapshotFiles(files);
  const progress = (event: PackProgressEvent) => {
    signal.throwIfAborted();
    onProgress?.(event);
    signal.throwIfAborted();
  };
  progress({ type: 'transferring_files_to_wasm' });
  if (fromSource) progress({ type: 'collecting_source_inputs' });
  const session = await openPackSession(snapshot, fromSource, signal, progress);
  try {
    const decisions: Record<string, ProgressiveAssetDecision> = Object.create(null);
    if (onProgressiveAsset) {
      for (const candidate of session.candidates) {
        signal.throwIfAborted();
        const id = candidate.id;
        let active = true;
        let plan: Promise<ProgressiveAssetPlan> | undefined;
        const prepare = async () => {
          signal.throwIfAborted();
          if (!active) throw new Error('Progressive Asset callback has finished');
          plan ??= session.prepare(id);
          void plan.catch(() => {});
          // The upload may mutate or transfer the caller's copy. The pack
          // session and subsequent prepare calls keep the original bytes.
          return structuredClone(await plan);
        };
        try {
          const decision = await untilAborted(signal, () => onProgressiveAsset(candidate, { signal, prepare }));
          if (!decision || !['embedded', 'progressive'].includes(decision.delivery)) {
            throw new Error('Invalid Progressive Asset delivery decision');
          }
          if (plan) await plan;
          decisions[id] = structuredClone(decision);
        } finally {
          active = false;
        }
      }
    }
    signal.throwIfAborted();
    return await session.finalize(decisions, buildId, engine, optimize, agentId);
  } finally {
    session.close();
  }
}

function untilAborted<T>(signal: AbortSignal, work: () => T | Promise<T>): Promise<T> {
  return new Promise((resolve, reject) => {
    if (signal.aborted) {
      reject(signal.reason);
      return;
    }
    const onAbort = () => reject(signal.reason);
    signal.addEventListener('abort', onAbort, { once: true });
    Promise.resolve().then(() => {
      signal.throwIfAborted();
      return work();
    }).then(resolve, reject).finally(() => signal.removeEventListener('abort', onAbort));
  });
}
