import ts from 'typescript';
import environment from './environment.generated.js';
import type { CheckDiagnostic, CheckOptions, PreparedCheck } from './types.js';

const ROOT = '/project/';
const LIB = '/__aix__/typescript/';
const INK = '/__aix__/node_modules/@yodaos-pkg/ink-env/index.d.ts';

/** No ambient filesystem, process, network, or compiler output is used here. */
export function checkTypeScript(prepared: PreparedCheck, onProgress?: CheckOptions['onProgress']): {
  diagnostics: CheckDiagnostic[];
  rootCount?: number;
} {
  if (!prepared.scripts.some((script) => script.typescript && !script.path.endsWith('.d.ts'))
    && !prepared.files.some((file) => file.path === 'tsconfig.json')) return { diagnostics: [] };

  onProgress?.({ type: 'configuring_typescript' });
  const diagnostics: CheckDiagnostic[] = [];
  const originals = new Map<string, { path: string; source: string }>();
  const files = new Map<string, string>(Object.entries(environment));
  let roots: string[] = [];
  const sourceNames = new Map<string, string>();
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
    if (script.path.endsWith('.ink')) files.set(resolve(ROOT, script.path), script.script);
    originals.set(name, script);
    sourceNames.set(resolve(ROOT, script.path), name);
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
  // Enumerate real input names, not the virtual .ink.ts names, when applying
  // tsconfig globs. Imports still resolve to the corresponding virtual modules.
  const entries = directoryEntries([...files.keys()].filter((file) => !originals.get(file)?.path.endsWith('.ink')
    || sourceNames.has(file)));
  const readDirectory: NonNullable<ts.CompilerHost['readDirectory']> = (directory, extensions, excludes, includes, depth) =>
    compilerMatchFiles(directory, extensions, excludes, includes, true, ROOT, depth,
      (directory) => entries.get(normalize(directory)) ?? { files: [], directories: [] }, normalize);
  const system: ts.ParseConfigHost & ts.ModuleResolutionHost = {
    useCaseSensitiveFileNames: true,
    fileExists: (file) => files.has(normalize(file)),
    readFile: (file) => files.get(normalize(file)),
    directoryExists: (directory) => directories.has(normalize(directory)),
    getDirectories: (directory) => [...directories].filter((entry) => dirname(entry) === normalize(directory)),
    readDirectory,
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
  let projectReferences: readonly ts.ProjectReference[] | undefined;
  const configPath = resolve(ROOT, 'tsconfig.json');
  if (system.fileExists(configPath)) {
    const config = ts.readJsonConfigFile(configPath, system.readFile);
    const parsed = ts.parseJsonSourceFileConfigFileContent(
      config, system, ROOT, undefined, configPath, undefined,
      // Deferred admits mixed-content extensions during root selection. The
      // selected Ink file is then mapped to its extracted .ts/.js virtual file.
      [{ extension: '.ink', scriptKind: ts.ScriptKind.Deferred, isMixedContent: true }],
    );
    options = { ...options, moduleDetection: ts.ModuleDetectionKind.Auto, ...parsed.options };
    if (parsed.options.module !== undefined && parsed.options.moduleResolution === undefined) delete options.moduleResolution;
    roots = parsed.fileNames.map((name) => sourceNames.get(normalize(name)) ?? name);
    projectReferences = parsed.projectReferences;
    diagnostics.push(...ts.getConfigFileParsingDiagnostics(parsed).map((error) => convert(error, 'AIX049')));
  }
  options = { ...options, noEmit: true, emitDeclarationOnly: false, allowJs: true, checkJs: false };
  const inkTypes = ts.resolveTypeReferenceDirective(
    '@yodaos-pkg/ink-env', resolve(ROOT, '__aix_types__.ts'), options, system,
  ).resolvedTypeReferenceDirective?.resolvedFileName ?? INK;

  const host: ts.CompilerHost = {
    ...system,
    readDirectory,
    getSourceFile(file, languageVersion) {
      const source = system.readFile(file);
      if (source === undefined) return undefined;
      // An SFC is an implicit module even when its script has no import/export.
      const text = originals.get(normalize(file))?.path.endsWith('.ink') ? source + '\nexport {};' : source;
      return ts.createSourceFile(file, text, languageVersion, true);
    },
    getDefaultLibFileName: (compilerOptions) => LIB + ts.getDefaultLibFileName(compilerOptions),
    getDefaultLibLocation: () => LIB,
    writeFile: () => {},
    getCurrentDirectory: () => ROOT,
    getCanonicalFileName: normalize,
    getNewLine: () => '\n',
    useCaseSensitiveFileNames: () => true,
    resolveModuleNameLiterals: (literals, containingFile, redirectedReference, compilerOptions, containingSourceFile) => literals.map((literal) => {
      const name = literal.text;
      if (name.endsWith('.ink') && name.startsWith('.')) {
        const base = resolve(dirname(containingFile), name);
        for (const extension of [ts.Extension.Ts, ts.Extension.Js]) {
          const resolvedFileName = base + extension;
          if (system.fileExists(resolvedFileName)) return { resolvedModule: { resolvedFileName, extension } };
        }
      }
      return ts.resolveModuleName(name, containingFile, compilerOptions, system, undefined, redirectedReference,
        ts.getModeForUsageLocation(containingSourceFile, literal, compilerOptions));
    }),
    resolveTypeReferenceDirectiveReferences: (names, containingFile, redirectedReference, compilerOptions, containingSourceFile) => names.map((entry) => {
      const name = typeof entry === 'string' ? entry : entry.fileName;
      const mode = (typeof entry === 'string' ? undefined : entry.resolutionMode) ?? containingSourceFile?.impliedNodeFormat;
      const result = ts.resolveTypeReferenceDirective(name, containingFile, compilerOptions, system, redirectedReference, undefined, mode);
      if (!result.resolvedTypeReferenceDirective && name === '@yodaos-pkg/ink-env') {
        return { ...result, resolvedTypeReferenceDirective: { resolvedFileName: inkTypes, primary: false } };
      }
      return result;
    }),
  };
  // One project Program owns the import graph and symbol table for all roots.
  onProgress?.({ type: 'resolving_typescript_project', rootCount: roots.length });
  const program = ts.createProgram({ rootNames: [...roots, inkTypes], options, host, projectReferences });
  onProgress?.({ type: 'checking_typescript', rootCount: roots.length });
  diagnostics.push(...program.getOptionsDiagnostics().map((error) => convert(error, 'AIX049')));
  diagnostics.push(...[
    ...program.getGlobalDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getSemanticDiagnostics(),
  ].map((error) => convert(error, 'AIX048')));
  return { diagnostics, rootCount: roots.length };

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

type DirectoryEntries = { files: string[]; directories: string[] };

// TypeScript uses this matcher for tsconfig itself. It is exposed by the pinned
// compiler runtime but omitted from typescript.d.ts; keep its signature local.
const compilerMatchFiles = (ts as typeof ts & {
  matchFiles(path: string, extensions: readonly string[] | undefined, excludes: readonly string[] | undefined,
    includes: readonly string[] | undefined, caseSensitive: boolean, currentDirectory: string, depth: number | undefined,
    getEntries: (path: string) => DirectoryEntries, realpath: (path: string) => string): string[];
}).matchFiles;

function directoryEntries(names: string[]): Map<string, DirectoryEntries> {
  const result = new Map<string, DirectoryEntries>();
  const entry = (directory: string) => {
    let value = result.get(directory);
    if (!value) result.set(directory, value = { files: [], directories: [] });
    return value;
  };
  for (const name of names) {
    let directory = dirname(name);
    entry(directory).files.push(name.slice(directory.length + (directory === '/' ? 0 : 1)));
    while (directory !== '/') {
      const parent = dirname(directory);
      const child = directory.slice(parent.length + (parent === '/' ? 0 : 1));
      if (!entry(parent).directories.includes(child)) entry(parent).directories.push(child);
      directory = parent;
    }
  }
  return result;
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
