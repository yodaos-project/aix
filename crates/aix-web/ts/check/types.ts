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
