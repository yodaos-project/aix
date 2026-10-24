import path from "node:path";
import ts from "typescript";
import type { CheckDiagnostic, CheckScriptSource } from "./wasm";

/** Type check the same normalized, ignore-filtered scripts as the Rust checker. */
export function checkTypeScript(input: string, scripts: CheckScriptSource[]): CheckDiagnostic[] {
  if (!scripts.some((script) => script.typescript && !script.path.endsWith(".d.ts"))) return [];

  const root = path.resolve(input);
  const diagnostics: CheckDiagnostic[] = [];
  const virtual = new Map<string, CheckScriptSource>();
  const canonical = (file: string) => {
    const absolute = path.resolve(file);
    return ts.sys.useCaseSensitiveFileNames ? absolute : absolute.toLowerCase();
  };
  for (const script of scripts) {
    const file = script.path.endsWith(".ink")
      ? `${script.path}.${script.typescript ? "ts" : "js"}`
      : script.path;
    virtual.set(canonical(path.join(root, file)), script);
  }

  let options: ts.CompilerOptions = {
    target: ts.ScriptTarget.ES2022,
    module: ts.ModuleKind.ESNext,
    moduleResolution: ts.ModuleResolutionKind.Bundler,
    moduleDetection: ts.ModuleDetectionKind.Force,
    lib: ["lib.es2022.d.ts"],
    types: [],
    strict: true,
    skipLibCheck: true,
  };
  const configPath = path.join(root, "tsconfig.json");
  if (ts.sys.fileExists(configPath)) {
    const config = ts.readConfigFile(configPath, ts.sys.readFile);
    if (config.error) {
      diagnostics.push(convert(config.error, "AIX049"));
    } else {
      // Roots come from the AIX collector, including inline Ink scripts. Read
      // compiler options (and extends) without requiring tsconfig include globs
      // to recognize .ink files or standalone source directories.
      const parsed = ts.parseJsonConfigFileContent(
        { ...config.config, files: undefined, include: [] }, ts.sys, root, undefined, configPath,
      );
      options = { ...options, ...parsed.options };
      if (parsed.options.module !== undefined && parsed.options.moduleResolution === undefined) {
        // Let TypeScript choose the matching resolution mode for an explicit
        // CommonJS/NodeNext module setting instead of imposing bundler mode.
        delete options.moduleResolution;
      }
      diagnostics.push(...parsed.errors.filter((error) => error.code !== 18003).map((error) => convert(error, "AIX049")));
    }
  }
  options = { ...options, noEmit: true, emitDeclarationOnly: false, allowJs: true, checkJs: false };

  const host = ts.createCompilerHost(options);
  const readFile = host.readFile.bind(host);
  const fileExists = host.fileExists.bind(host);
  const isExcludedSource = (file: string) => {
    const relative = path.relative(root, path.resolve(file));
    return relative !== ".." && !relative.startsWith(`..${path.sep}`) && !path.isAbsolute(relative)
      && !relative.split(path.sep).includes("node_modules") && /\.(?:[cm]?js|[cm]?ts|ink)$/.test(file);
  };
  host.readFile = (file) => virtual.get(canonical(file))?.script
    ?? (isExcludedSource(file) ? undefined : readFile(file));
  host.fileExists = (file) => virtual.has(canonical(file))
    || (!isExcludedSource(file) && fileExists(file));
  host.getSourceFile = (file, languageVersion) => {
    const content = host.readFile(file);
    if (content === undefined) return undefined;
    return ts.createSourceFile(file, content, languageVersion, true);
  };
  host.getCurrentDirectory = () => root;
  host.resolveModuleNames = (names, containingFile) => names.map((name) => {
    if (name.endsWith(".ink") && name.startsWith(".")) {
      const base = path.resolve(path.dirname(containingFile), name);
      for (const extension of [ts.Extension.Ts, ts.Extension.Js]) {
        const resolvedFileName = `${base}${extension}`;
        if (virtual.has(canonical(resolvedFileName))) return { resolvedFileName, extension };
      }
    }
    return ts.resolveModuleName(name, containingFile, options, host).resolvedModule;
  });

  // Prefer the project's runtime declarations; a clean project also works
  // through the CLI's installed Ink environment package, without network I/O.
  const inkTypes = ts.resolveTypeReferenceDirective(
    "@yodaos-pkg/ink-env", path.join(root, "__aix_types__.ts"), options, host,
  ).resolvedTypeReferenceDirective?.resolvedFileName ?? require.resolve("@yodaos-pkg/ink-env/index.d.ts");
  host.resolveTypeReferenceDirectives = (names, containingFile) => names.map((entry) => {
    const name = typeof entry === "string" ? entry : entry.fileName;
    return ts.resolveTypeReferenceDirective(name, containingFile, options, host).resolvedTypeReferenceDirective
      ?? (name === "@yodaos-pkg/ink-env" ? { resolvedFileName: inkTypes, primary: false } : undefined);
  });
  const roots = scripts.filter((script) => script.typescript)
    .map((script) => path.join(root, script.path.endsWith(".ink") ? `${script.path}.ts` : script.path));
  const program = ts.createProgram([...roots, inkTypes], options, host);
  diagnostics.push(...program.getOptionsDiagnostics().map((error) => convert(error, "AIX049")));
  const errors = [
    ...program.getGlobalDiagnostics(),
    ...program.getSyntacticDiagnostics(),
    ...program.getSemanticDiagnostics(),
  ];
  diagnostics.push(...errors.map((error) => convert(error, "AIX048")));
  return diagnostics;

  function convert(diagnostic: ts.Diagnostic, code: string): CheckDiagnostic {
    let file = "tsconfig.json";
    let line = 1;
    let column = 1;
    if (diagnostic.file) {
      const script = virtual.get(canonical(diagnostic.file.fileName));
      file = script?.path ?? path.relative(root, diagnostic.file.fileName).split(path.sep).join("/");
      const source = script?.source ?? diagnostic.file.text;
      const before = source.slice(0, diagnostic.start ?? 0);
      line = before.split("\n").length;
      column = Array.from(before.slice(before.lastIndexOf("\n") + 1)).length + 1;
    }
    return {
      code,
      severity: diagnostic.category === ts.DiagnosticCategory.Error ? "error" : "warning",
      path: file,
      line,
      column,
      message: `TS${diagnostic.code}: ${ts.flattenDiagnosticMessageText(diagnostic.messageText, "\n")}`,
    };
  }
}
