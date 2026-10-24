import type { CheckDiagnostic, CheckResult, PreparedCheck } from './types.js';

export function sourceResults(prepared: PreparedCheck): CheckResult[] {
  const { coverage, report } = prepared;
  const groups = new Map<CheckResult['id'], CheckDiagnostic[]>();
  for (const diagnostic of report.diagnostics) {
    const id = sourceCategory(diagnostic.code);
    let group = groups.get(id);
    if (!group) groups.set(id, group = []);
    group.push(diagnostic);
  }
  const blocked = coverage.structure ? undefined : 'app.json could not be validated';
  const inputErrors = groups.get('json')?.some((diagnostic) => diagnostic.severity === 'error');
  const syntaxErrors = groups.get('syntax')?.some((diagnostic) => diagnostic.severity === 'error');
  return [
    result('json', `JSON syntax and text encoding (${count(coverage.text_files, 'file')})`, groups.get('json'),
      coverage.text_files ? undefined : 'no text source files'),
    result('structure', 'App, Page, Widget, and component structure', groups.get('structure'),
      blocked ?? (inputErrors ? 'some source files could not be parsed' : undefined)),
    result('syntax', `JS/TS syntax (${count(coverage.scripts, 'script')})`, groups.get('syntax'),
      blocked ?? (coverage.scripts ? undefined : 'no scripts to check')),
    result('permissions', 'Permission declarations and direct API use', groups.get('permissions'),
      blocked ?? (inputErrors || syntaxErrors ? 'some source files could not be parsed' : undefined)),
    result('schemas', `Page JSON Schemas (${count(coverage.schemas, 'schema')})`, groups.get('schemas'),
      blocked ?? (coverage.schemas ? undefined : 'no Page schemas checked')),
  ];
}

export function typeScriptResults(diagnostics: CheckDiagnostic[], rootCount?: number): CheckResult[] {
  const config = diagnostics.filter((diagnostic) => diagnostic.code === 'AIX049');
  const skipped = rootCount === undefined ? 'no TypeScript project' : undefined;
  const blocked = skipped ?? (config.some((diagnostic) => diagnostic.severity === 'error')
    ? 'invalid TypeScript configuration' : rootCount === 0 ? 'no TypeScript project roots' : undefined);
  return [
    result('typescript_config', 'TypeScript project configuration', config, skipped),
    result('typescript', `TypeScript project types and imports (${count(rootCount ?? 0, 'root')})`,
      diagnostics.filter((diagnostic) => diagnostic.code === 'AIX048'), blocked),
  ];
}

function result(id: CheckResult['id'], label: string, diagnostics: CheckDiagnostic[] = [], detail?: string): CheckResult {
  const errors = diagnostics.filter((diagnostic) => diagnostic.severity === 'error').length;
  const warnings = diagnostics.length - errors;
  return { id, label, errors, warnings,
    status: errors ? 'failed' : warnings ? 'warning' : detail ? 'skipped' : 'passed',
    ...(detail ? { detail } : {}),
  };
}

function sourceCategory(code: string): CheckResult['id'] {
  if (['AIX001', 'AIX002', 'AIX014'].includes(code)) return 'json';
  if (['AIX006', 'AIX007', 'AIX027', 'AIX028', 'AIX033', 'AIX037', 'AIX038'].includes(code)) return 'permissions';
  if (code === 'AIX029') return 'syntax';
  if (['AIX031', 'AIX047'].includes(code)) return 'schemas';
  return 'structure';
}

function count(value: number, unit: string): string {
  return `${value} ${unit}${value === 1 ? '' : 's'}`;
}
