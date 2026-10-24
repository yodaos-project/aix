import { checkTypeScript } from './typescript.js';
import { sourceResults, typeScriptResults } from './results.js';
import type { CheckEngine, CheckOptions, CheckReport } from './types.js';

export type { CheckDiagnostic, CheckOptions, CheckProgressEvent, CheckReport, CheckResult } from './types.js';

/** The complete check pipeline shared by the Web SDK and npm CLI. */
export function checkSource(
  files: { path: string; data: Uint8Array }[],
  engine: CheckEngine,
  options: CheckOptions = {},
): CheckReport {
  options.onProgress?.({ type: 'checking_sources', fileCount: files.length });
  const prepared = engine.prepare_aix_check(files);
  for (const check of sourceResults(prepared)) options.onProgress?.({ type: 'check_finished', check });
  const typescript = checkTypeScript(prepared, options);
  for (const check of typeScriptResults(typescript.diagnostics, typescript.rootCount)) {
    options.onProgress?.({ type: 'check_finished', check });
  }
  const diagnostics = [...prepared.report.diagnostics, ...typescript.diagnostics];
  options.onProgress?.({ type: 'collecting_diagnostics' });
  diagnostics.sort((a, b) => compare(a.path, b.path)
    || a.line - b.line || a.column - b.column || compare(a.code, b.code));
  return { diagnostics };
}

// Compare Unicode scalar values, matching UTF-8 byte order in the Rust checker.
function compare(a: string, b: string): number {
  const left = Array.from(a, (ch) => ch.codePointAt(0)!);
  const right = Array.from(b, (ch) => ch.codePointAt(0)!);
  for (let i = 0; i < Math.min(left.length, right.length); i++) {
    if (left[i] !== right[i]) return left[i] - right[i];
  }
  return left.length - right.length;
}
