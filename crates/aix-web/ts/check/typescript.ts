import ts from 'typescript';
import environment from './environment.generated.js';
import type { CheckDiagnostic, PreparedCheck } from './types.js';

const ROOT = '/project/';
const LIB = '/__aix__/typescript/';
const INK = '/__aix__/node_modules/@yodaos-pkg/ink-env/index.d.ts';

/** No ambient filesystem, process, network, or compiler output is used here. */
export function checkTypeScript(prepared: PreparedCheck): CheckDiagnostic[] {
  if (!prepared.scripts.some((script) => script.typescript && !script.path.endsWith('.d.ts'))) return [];

  const diagnostics: CheckDiagnostic[] = [];
  const originals = new Map<string, { path: string; source: string }>();
  const files = new Map<string, string>(Object.entries(environment));
  const roots: string[] = [];
  for (const file of prepared.files) {
    const name = resolve(ROOT, file.path);
    // Configs and installed dependencies can be used even when not packaged.
    if (file.path.endsWith('.json') || file.path.split('/').includes('node_modules')) files.set(name, file.source);
    originals.set(name, file);
  }
  for (const script of prepared.scripts) {
    const suffix = script.path.endsWith('.ink') ? (script.typescript ? '.ts' : '.js') : '';
    const name = resolve(ROOT, script.path + suffix);
    files.set(name, script.script);
    originals.set(name, script);
    if (script.typescript) roots.push(name);
  }
  const directories = new Set<string>(['/']);
  for (const file of files.keys()) {
    let directory = dirname(file);
    while (directory !== '/') {
      directories.add(directory);
      directory = dirname(directory);
    }
  }
  const system: ts.ParseConfigHost & ts.ModuleResolutionHost = {
    useCaseSensitiveFileNames: true,
    fileExists: (file) => files.has(normalize(file)),
    readFile: (file) => files.get(normalize(file)),
    directoryExists: (directory) => directories.has(normalize(directory)),
    getDirectories: (directory) => [...directories].filter((entry) => dirname(entry) === normalize(directory)),
    readDirectory: () => [], // Check roots always come from the AIX collector.
    realpath: normalize,
    getCurrentDirectory: () => ROOT,
  };
  let options: ts.CompilerOptions = {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    moduleDetection: ts.ModuleDetectionKind.Force,
    lib: ['lib.es2022.d.ts'],
    types: [],
    strict: true,
    skipLibCheck: true,
  };
  const configPath = resolve(ROOT, 'tsconfig.json');
  if (system.fileExists(configPath)) {
    const config = ts.readConfigFile(configPath, system.readFile);
    if (config.error) {
      diagnostics.push(convert(config.error, 'AIX049'));
    } else {
      const parsed = ts.parseJsonConfigFileContent(
        { ...config.config, files: undefined, include: [] }, system, ROOT, undefined, configPath,
      );
      options = { ...options, ...parsed.options };
      if (parsed.options.module !== undefined && parsed.options.moduleResolution === undefined) delete options.moduleResolution;
      diagnostics.push(...parsed.errors.filter((error) => error.code !== 18003).map((error) => convert(error, 'AIX049')));
    }
  }
  options = { ...options, noEmit: true, emitDeclarationOnly: false, allowJs: true, checkJs: false };
  const inkTypes = ts.resolveTypeReferenceDirective(
    '@yodaos-pkg/ink-env', resolve(ROOT, '__aix_types__.ts'), options, system,
  ).resolvedTypeReferenceDirective?.resolvedFileName ?? INK;

  const host: ts.CompilerHost = {
    ...system,
    readDirectory: () => [],
    getSourceFile(file, languageVersion) {
      const source = system.readFile(file);
      return source === undefined ? undefined : ts.createSourceFile(file, source, languageVersion, true);
    },
    getDefaultLibFileName: (compilerOptions) => LIB + ts.getDefaultLibFileName(compilerOptions),
    getDefaultLibLocation: () => LIB,
    writeFile: () => {},
    getCurrentDirectory: () => ROOT,
    getCanonicalFileName: normalize,
    getNewLine: () => '\n',
    useCaseSensitiveFileNames: () => true,
    resolveModuleNames: (names, containingFile) => names.map((name) => {
      if (name.endsWith('.ink') && name.startsWith('.')) {
        const base = resolve(dirname(containingFile), name);
        for (const extension of [ts.Extension.Ts, ts.Extension.Js]) {
          const resolvedFileName = base + extension;
          if (system.fileExists(resolvedFileName)) return { resolvedFileName, extension };
        }
      }
      return ts.resolveModuleName(name, containingFile, options, system).resolvedModule;
    }),
    resolveTypeReferenceDirectives: (names, containingFile) => names.map((entry) => {
      const name = typeof entry === 'string' ? entry : entry.fileName;
      return ts.resolveTypeReferenceDirective(name, containingFile, options, system).resolvedTypeReferenceDirective
        ?? (name === '@yodaos-pkg/ink-env' ? { resolvedFileName: inkTypes, primary: false } : undefined);
    }),
  };
  const program = ts.createProgram([...roots, inkTypes], options, host);
  diagnostics.push(...program.getOptionsDiagnostics().map((error) => convert(error, 'AIX049')));
  diagnostics.push(...[
    ...program.getGlobalDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getSemanticDiagnostics(),
  ].map((error) => convert(error, 'AIX048')));
  return diagnostics;

  function convert(diagnostic: ts.Diagnostic, code: string): CheckDiagnostic {
    let file = 'tsconfig.json';
    let line = 1;
    let column = 1;
    if (diagnostic.file) {
      const name = normalize(diagnostic.file.fileName);
      const original = originals.get(name);
      file = original?.path ?? displayPath(name);
      const source = original?.source ?? diagnostic.file.text;
      const before = source.slice(0, diagnostic.start ?? 0);
      line = before.split('\n').length;
      column = Array.from(before.slice(before.lastIndexOf('\n') + 1)).length + 1;
    }
    return {
      code,
      severity: diagnostic.category === ts.DiagnosticCategory.Error ? 'error' : 'warning',
      path: file,
      line,
      column,
      message: `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, '\n').split(ROOT).join('')}`,
    };
  }
}

function displayPath(file: string): string {
  return file.startsWith(ROOT) ? file.slice(ROOT.length) : file;
}

function dirname(file: string): string {
  return file.slice(0, file.lastIndexOf('/')) || '/';
}

function resolve(directory: string, file: string): string {
  return normalize(file.startsWith('/') ? file : `${directory}/${file}`);
}

function normalize(file: string): string {
  const parts: string[] = [];
  for (const part of file.replace(/\\/g, '/').split('/')) {
    if (part === '..') parts.pop();
    else if (part && part !== '.') parts.push(part);
  }
  return '/' + parts.join('/');
}
