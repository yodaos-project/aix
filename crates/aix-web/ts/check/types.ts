export interface CheckDiagnostic {
  code: string;
  severity: 'error' | 'warning';
  path: string;
  line: number;
  column: number;
  message: string;
}

export interface CheckReport {
  diagnostics: CheckDiagnostic[];
}

export interface CheckResult {
  id: 'json' | 'structure' | 'syntax' | 'permissions' | 'schemas' | 'typescript_config' | 'typescript';
  label: string;
  status: 'passed' | 'warning' | 'failed' | 'skipped';
  errors: number;
  warnings: number;
  detail?: string;
}

export type CheckProgressEvent =
  | { type: 'loading_checker' }
  | { type: 'checking_sources'; fileCount: number }
  | { type: 'configuring_typescript' }
  | { type: 'resolving_typescript_project'; rootCount: number }
  | { type: 'checking_typescript'; rootCount: number }
  | { type: 'check_finished'; check: CheckResult }
  | { type: 'collecting_diagnostics' };

export interface CheckOptions {
  onProgress?: (event: CheckProgressEvent) => void;
  /** Developer-supplied declarations/package metadata, relative to the project root.
   * These files are used only by TypeScript, never packaged or source-checked.
   * A supplied package.json replaces that package's installed type snapshot.
   */
  typeFiles?: { path: string; source: string }[];
  /** Override tsconfig compilerOptions.types. An empty array disables automatic Ink types. */
  types?: string[];
}

export interface CheckScriptSource {
  path: string;
  source: string;
  script: string;
  typescript: boolean;
}

export interface PreparedCheck {
  report: CheckReport;
  scripts: CheckScriptSource[];
  files: { path: string; source: string }[];
  coverage: { text_files: number; structure: boolean; scripts: number; schemas: number };
}

export interface CheckEngine {
  prepare_aix_check(files: { path: string; data: Uint8Array }[]): PreparedCheck;
}
