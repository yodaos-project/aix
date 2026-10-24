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

export type CheckProgressEvent =
  | { type: 'loading_checker' }
  | { type: 'checking_sources'; fileCount: number }
  | { type: 'configuring_typescript' }
  | { type: 'resolving_typescript_project'; rootCount: number }
  | { type: 'checking_typescript'; rootCount: number }
  | { type: 'collecting_diagnostics' };

export interface CheckOptions {
  onProgress?: (event: CheckProgressEvent) => void;
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
}

export interface CheckEngine {
  prepare_aix_check(files: { path: string; data: Uint8Array }[]): PreparedCheck;
}
