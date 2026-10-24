import { checkTypeScript } from './typescript.js';
import type { CheckEngine, CheckReport } from './types.js';

export type { CheckDiagnostic, CheckReport } from './types.js';

/** The complete check pipeline shared by the Web SDK and npm CLI. */
export function checkSource(
  files: { path: string; data: Uint8Array }[],
  engine: CheckEngine,
): CheckReport {
  const prepared = engine.prepare_aix_check(files);
  const diagnostics = [...prepared.report.diagnostics, ...checkTypeScript(prepared)];
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
