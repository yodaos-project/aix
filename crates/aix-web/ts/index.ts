import init, {
  AixReaderWasm,
  optimize_aix,
  pack_aix,
} from '../dist/pkg/aix_web.js';
import * as wasmExports from '../dist/pkg/aix_web.js';

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
}

export interface PackOptions {
  buildId?: string;
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
) => { data: Uint8Array; report: unknown; warnings?: string[] };

type PackFromSourceWithProgressFn = (
  files: AixInputFile[],
  buildId: string,
  engine: string | undefined,
  optimize: false | OptimizeOptions | undefined,
  progress: (event: PackProgressEvent) => void,
) => { data: Uint8Array; report: unknown; warnings?: string[] };

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

export interface PageInfo {
  name: string;
  title?: string;
  data_schema: any;
}

export interface WidgetInfo {
  path: string;
  family: string;
  placement: 'persistent' | 'overlay';
}

export interface Tool {
  type: string;
  function: {
    name: string;
    description?: string;
    parameters: any;
  };
}

export class AIX {
  private reader: AixReaderWasm;

  private constructor(reader: AixReaderWasm) {
    this.reader = reader;
  }

  /**
   * Initialize the WASM module and create an AIX instance from the given data.
   * @param data The .aix file content as Uint8Array or File
   */
  static async From(data: Uint8Array | File): Promise<AIX> {
    await init();
    let buffer: Uint8Array;
    if (data instanceof Uint8Array) {
      buffer = data;
    } else {
      const arrayBuffer = await data.arrayBuffer();
      buffer = new Uint8Array(arrayBuffer);
    }
    const reader = new AixReaderWasm(buffer);
    return new AIX(reader);
  }

  static async pack(
    files: AixInputFile[],
    options?: PackOptions,
  ): Promise<PackResult> {
    await init();
    const buildId = options?.buildId ?? generateBuildId();
    const optimize = options?.optimize === false ? undefined : options?.optimize;
    const result = pack_aix(files, buildId, options?.engine, optimize);
    return { data: result.data, report: result.report as OptimizeReport, warnings: result.warnings ?? [] };
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
    await init();
    const buildId = options?.buildId ?? generateBuildId();
    const optimize = options?.optimize === false ? undefined : options?.optimize;
    const result = options?.onProgress
      ? getPackFromSourceWithProgress()(
          files,
          buildId,
          options.engine,
          optimize,
          options.onProgress,
        )
      : getPackFromSource()(files, buildId, options?.engine, optimize);
    return { data: result.data, report: result.report as OptimizeReport, warnings: result.warnings ?? [] };
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
    await init();
    const buffer = data instanceof Uint8Array
      ? data
      : new Uint8Array(await data.arrayBuffer());
    const result = optimize_aix(buffer, options);
    return { data: result.data, report: result.report as OptimizeReport, warnings: result.warnings ?? [] };
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

  supportsEngine(currentVersion: string): boolean {
    return this.reader.supports_engine(currentVersion);
  }

  /**
   * Get the title from app.json.
   */
  getTitle(): string | undefined {
    return (this.reader as any).get_title();
  }

  /**
   * Get all pages from app.json and pages/*.json.
   */
  getPages(): PageInfo[] {
    return (this.reader as any).get_pages() as PageInfo[];
  }

  /**
   * Get all widgets from app.json after validating that each .ink entry exists.
   */
  getWidgets(): WidgetInfo[] {
    return (this.reader as any).get_widgets() as WidgetInfo[];
  }

  /**
   * Get all tools from app.json and pages/*.json in OpenAI format.
   */
  getTools(): Tool[] {
    return (this.reader as any).get_tools() as Tool[];
  }
}

function resolveWebFilePath(file: File): string {
  const candidate = (file as File & { webkitRelativePath?: string }).webkitRelativePath;
  return candidate && candidate.length > 0 ? candidate : file.name;
}

function getPackFromSource(): PackFromSourceFn {
  const packFromSource = (wasmExports as typeof wasmExports & {
    pack_aix_from_source?: PackFromSourceFn;
  }).pack_aix_from_source;
  if (!packFromSource) {
    throw new Error('pack_aix_from_source is not available in the current WASM bundle');
  }
  return packFromSource;
}

function getPackFromSourceWithProgress(): PackFromSourceWithProgressFn {
  const packFromSourceWithProgress = (wasmExports as typeof wasmExports & {
    pack_aix_from_source_with_progress?: PackFromSourceWithProgressFn;
  }).pack_aix_from_source_with_progress;
  if (!packFromSourceWithProgress) {
    throw new Error('pack_aix_from_source_with_progress is not available in the current WASM bundle');
  }
  return packFromSourceWithProgress;
}
