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
  id: 'json' | 'structure' | 'syntax' | 'permissions' | 'schemas';
  label: string;
  status: 'passed' | 'warning' | 'failed' | 'skipped';
  errors: number;
  warnings: number;
  detail?: string;
}

export type CheckProgressEvent =
  | { type: 'loading_checker' }
  | { type: 'checking_sources'; fileCount: number }
  | { type: 'check_finished'; check: CheckResult }
  | { type: 'collecting_diagnostics' };

export interface CheckOptions {
  onProgress?: (event: CheckProgressEvent) => void;
}

export interface PreparedCheck {
  report: CheckReport;
  coverage: { text_files: number; structure: boolean; scripts: number; schemas: number };
}

export interface CheckEngine {
  prepare_aix_check(files: { path: string; data: Uint8Array }[]): PreparedCheck;
}
