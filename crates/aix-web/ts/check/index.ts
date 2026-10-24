import { sourceResults } from './results.js';
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
  options.onProgress?.({ type: 'collecting_diagnostics' });
  return prepared.report;
}
