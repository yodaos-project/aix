import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { spawn } from "node:child_process";
import { select } from "@inquirer/prompts";
import ora from "ora";
import { cmdPack } from "./legacy";
import { loadEngine } from "../wasm";

const DEVELOP_URI = "content://com.rokid.aiui.develop";
const STAGING_DIR = "/sdcard/aiui/package/.staging/adb/";
const PACKAGE_DIR = "/sdcard/aiui/package";
const MAX_PACKAGE_BYTES = 128 * 1024 * 1024;

type Definition = Record<string, unknown> & { agentId?: unknown };
type DevelopResult = Record<string, unknown>;

async function runStep<T>(progress: string, success: string | undefined, task: () => T | Promise<T>, complete: (result: T) => boolean = () => true): Promise<T> {
  const spinner = process.stderr.isTTY ? ora({ text: progress, stream: process.stderr }).start() : undefined;
  try {
    const result = await task();
    if (complete(result)) {
      if (success) {
        if (spinner) spinner.succeed(success);
        else process.stdout.write(`✔ ${success}\n`);
      } else {
        spinner?.stop();
      }
    } else {
      spinner?.stop();
    }
    return result;
  } catch (error) {
    spinner?.stop();
    throw error;
  }
}

export type InstallOptions = {
  definition?: string;
  serial?: string;
  optimize?: boolean;
  optLevel: string;
  engine?: string;
};

export type ResolvedDefinition = {
  input: string;
  isProject: boolean;
  definition: Definition;
  app: Definition;
};

export function resolveDefinition(inputValue: string, definitionOverride?: string): ResolvedDefinition {
  const input = fs.realpathSync(path.resolve(inputValue));
  const inputStat = fs.statSync(input);
  const isProject = inputStat.isDirectory();
  if (!isProject && (!inputStat.isFile() || path.extname(input).toLowerCase() !== ".aix")) {
    throw new Error(`Input must be a project directory or .aix file: ${input}`);
  }
  const metadata = readLaunchMetadata(input, isProject);
  const definitionPath = definitionOverride
    ? fs.realpathSync(path.resolve(definitionOverride))
    : isProject && fs.existsSync(path.join(input, "agent.json"))
      ? path.join(input, "agent.json")
      : undefined;
  const definition = definitionPath
    ? (JSON.parse(fs.readFileSync(definitionPath, "utf8")) as Definition)
    : buildDefinition(metadata.app, metadata.agentId, metadata.agentsText);
  definition.agentId = metadata.agentId;
  validateDefinition(definition);
  return { input, isProject, definition, app: metadata.app };
}

export type PageLaunchOptions = {
  card?: boolean;
  params?: string;
  paramsFile?: string;
  serial?: string;
};

export async function cmdLaunchPage(inputValue: string, route: string | undefined, options: PageLaunchOptions) {
  const resolved = resolveDefinition(inputValue);
  const agentId = validateDefinition(resolved.definition);
  const { path: launchPath } = resolveOpenRoute(resolved.app, route, "blank");
  const params = parseOpenParams(options.params, options.paramsFile);
  const serial = await selectDevice(options.serial);
  const target = options.card ? "_current" : "_blank";
  const request = { agentId, path: launchPath, target, params };
  const result = await runStep(`Launching ${options.card ? "Card" : "Page"}`, `${options.card ? "Card" : "Page"} launch requested`, () => performOpenRequest(serial, request), (value) => {
    requireOk(value, "open");
    return true;
  });
  finishOpenRequest(serial, request, result, false);
}

async function performOpenRequest(serial: string, request: Record<string, unknown>): Promise<DevelopResult> {
  return parseDevelopResult(
    await runOpenContentCall(serial, JSON.stringify(request)),
  );
}

function finishOpenRequest(serial: string, request: Record<string, unknown>, result: DevelopResult, printSuccess = true): void {
  requireOk(result, "open");
  const target = request.target === "_widget" ? "Widget" : request.target === "_current" ? "Card" : "Page";
  process.stdout.write([
    ...(printSuccess ? [`✔ ${target} launch requested`] : []),
    `  Agent: ${String(request.agentId)}`,
    `  Path: ${String(request.path)}`,
    `  Device: ${serial}`,
    "  Display: request accepted; rendering is handled asynchronously",
  ].join("\n") + "\n");
}

export async function cmdDevice(action: string | undefined, serialOption?: string) {
  if (action !== undefined && action !== "set-dev" && action !== "unset-dev") {
    throw new Error("Device action must be set-dev or unset-dev.");
  }
  const serial = await selectDevice(serialOption);
  if (!action) {
    const snapshot = await runStep("Reading device status", undefined, () => readWidgetSnapshot(serial));
    await printDeviceStatus(serial, snapshot);
    return;
  }
  const enabled = action === "set-dev";
  const mode = enabled ? "dev" : "prod";
  await runStep(
    `${enabled ? "Enabling" : "Disabling"} Developer Mode`,
    `Developer Mode ${enabled ? "enabled" : "disabled"}`,
    async () => parseDevelopResult(await runAdb(serial, contentCall("switch", mode))),
    (value) => {
      requireOk(value, "switch");
      return true;
    },
  );
  process.stdout.write([
    `  Device: ${serial}`,
    "  Widgets: reloaded; dynamic Widgets must be launched again",
  ].join("\n") + "\n");
}

async function printDeviceStatus(serial: string, snapshot: DevelopResult): Promise<void> {
  const environment = typeof snapshot.environment === "string" ? snapshot.environment : "unknown";
  const ready = snapshot.ready === true ? "ready" : "not ready";
  const version = typeof snapshot.configurationVersion === "string" && snapshot.configurationVersion
    ? snapshot.configurationVersion
    : "none";
  const epoch = typeof snapshot.environmentEpoch === "string" || typeof snapshot.environmentEpoch === "number"
    ? String(snapshot.environmentEpoch)
    : "unknown";
  const config = readWidgetConfig(snapshot);
  const lines = [
    `Device: ${serial}`,
    `Developer Mode: ${environment === "dev" ? "enabled" : environment === "prod" ? "disabled" : "unknown"}`,
    `Widget Runtime: ${ready}`,
    `Environment Epoch: ${epoch}`,
    `Configuration Version: ${version}`,
  ];
  if (!config) {
    lines.push("Widget Layout: not configured");
  } else {
    lines.push(
      `Widget Layout: ${String(config.totalColumnCount ?? "?")} columns, ${String(config.totalGridCount ?? "?")} cells`,
      ...formatWidgetGroup("Permanent Widgets", config.permanentModules),
      ...formatWidgetGroup("Dynamic Widget Placements", config.overlayModules),
    );
  }
  lines.push(...formatInstalledAgents(await readInstalledAgents(serial)));
  process.stdout.write(`${lines.join("\n")}\n`);
}

type InstalledAgent = Definition & { fileName: string; fileBytes: number };

async function readInstalledAgents(serial: string): Promise<InstalledAgent[]> {
  const filesOutput = await runAdb(serial, [
    "shell",
    `find ${PACKAGE_DIR} -maxdepth 1 -type f -name '*.aix' -exec stat -c '%n|%s' '{}' ';'`,
  ]);
  const files = new Map<string, number>();
  for (const line of filesOutput.split(/\r?\n/)) {
    const separator = line.lastIndexOf("|");
    if (separator < 0) continue;
    const fileName = path.posix.basename(line.slice(0, separator));
    const fileBytes = Number(line.slice(separator + 1));
    if (fileName.endsWith(".aix") && Number.isSafeInteger(fileBytes) && fileBytes >= 0) {
      files.set(fileName, fileBytes);
    }
  }
  const manifestOutput = (await runAdb(serial, [
    "shell",
    `if [ -f ${PACKAGE_DIR}/developer_manifest.json ]; then cat ${PACKAGE_DIR}/developer_manifest.json; fi`,
  ])).trim();
  let manifestPackages: Definition[] = [];
  if (manifestOutput) {
    const manifest = JSON.parse(manifestOutput) as Definition;
    manifestPackages = Array.isArray(manifest.packages)
      ? manifest.packages.filter((entry): entry is Definition => Boolean(entry) && typeof entry === "object" && !Array.isArray(entry))
      : [];
  }
  const metadata = new Map(
    manifestPackages
      .filter((entry) => typeof entry.fileName === "string")
      .map((entry) => [entry.fileName as string, entry]),
  );
  return [...files.entries()]
    .sort(([left], [right]) => left.localeCompare(right))
    .map(([fileName, fileBytes]) => ({ ...(metadata.get(fileName) ?? {}), fileName, fileBytes }));
}

function formatInstalledAgents(agents: InstalledAgent[]): string[] {
  const lines = [`Installed Agents (${agents.length}):`];
  if (agents.length === 0) return [...lines, "  none"];
  for (const agent of agents) {
    const name = typeof agent.agentName === "string" && agent.agentName ? agent.agentName : path.posix.basename(agent.fileName, ".aix");
    const id = typeof agent.agentId === "string" && agent.agentId ? agent.agentId : "unknown";
    lines.push(`  ${name}`);
    lines.push(`    ID: ${id}`);
    lines.push(`    Package: ${agent.fileName} (${formatFileSize(agent.fileBytes)})`);
    const versions = [
      typeof agent.nativeVersion === "string" && agent.nativeVersion ? `native ${agent.nativeVersion}` : undefined,
      typeof agent.inkVersion === "string" && agent.inkVersion ? `Ink ${agent.inkVersion}` : undefined,
    ].filter(Boolean);
    if (versions.length) lines.push(`    Runtime: ${versions.join(", ")}`);
  }
  return lines;
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KiB`;
  return `${(bytes / (1024 * 1024)).toFixed(1)} MiB`;
}

function formatWidgetGroup(title: string, modules: unknown[] | undefined): string[] {
  const entries = Array.isArray(modules) ? modules : [];
  const lines = [`${title} (${entries.length}):`];
  if (entries.length === 0) return [...lines, "  none"];
  for (const entry of entries) {
    const module = entry && typeof entry === "object" && !Array.isArray(entry) ? entry as Definition : {};
    const name = typeof module.name === "string" ? module.name : "unnamed";
    const agentId = typeof module.agentId === "string" ? module.agentId : "unknown agent";
    const widgetPath = typeof module.path === "string" ? module.path : "unknown path";
    lines.push(`  ${name}: ${agentId} -> ${widgetPath}`);
    lines.push(`    position ${String(module.startIndex ?? "?")}, size ${String(module.columnSize ?? "?")}x${String(module.rowSize ?? "?")}`);
  }
  return lines;
}

async function readWidgetSnapshot(serial: string): Promise<DevelopResult> {
  const result = parseDevelopResult(await runAdb(serial, contentCall("widget-snapshot")));
  requireOk(result, "widget-snapshot");
  return result;
}

type WidgetConfig = Record<string, unknown> & {
  totalGridCount?: number;
  totalColumnCount?: number;
  permanentModules?: unknown[];
  overlayModules?: unknown[];
};

function readWidgetConfig(snapshot: DevelopResult): WidgetConfig | undefined {
  const raw = snapshot.configurationJson;
  if (raw == null || raw === "") return undefined;
  const value = typeof raw === "string" ? JSON.parse(raw) : raw;
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("configurationJson must contain a JSON object.");
  }
  return value as WidgetConfig;
}

function emptyWidgetConfig(): WidgetConfig {
  return { totalGridCount: 4, totalColumnCount: 2, moduleBorderStyle: 0, permanentModules: [], overlayModules: [] };
}

async function prepareInbox(serial: string): Promise<string> {
  const result = parseDevelopResult(await runAdb(serial, contentCall("prepare")));
  requireEmptyErrorCode(result, "prepare");
  if (result.ready !== true) throw new Error("Prepare did not report ready=true.");
  return typeof result.inboxPath === "string" && result.inboxPath
    ? result.inboxPath
    : STAGING_DIR.replace(/\/$/, "");
}

async function applyWidgetConfig(serial: string, config: WidgetConfig): Promise<DevelopResult> {
  const inbox = await prepareInbox(serial);
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "aix-widget-layout-"));
  const local = path.join(temporary, "widget-config.json");
  try {
    const data = `${JSON.stringify(config, null, 2)}\n`;
    if (Buffer.byteLength(data) > 64 * 1024) throw new Error("Generated widget-config.json is larger than 64 KiB.");
    fs.writeFileSync(local, data);
    await runAdb(serial, ["push", local, `${inbox.replace(/\/$/, "")}/widget-config.json`]);
    const result = parseDevelopResult(await runAdb(serial, contentCall("widget-apply")));
    requireOk(result, "widget-apply");
    return result;
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

export async function cmdWidgetLayout(clear: boolean, serialOption?: string) {
  const serial = await selectDevice(serialOption);
  const snapshot = await runStep("Reading Widget layout", undefined, () => readWidgetSnapshot(serial));
  const config = readWidgetConfig(snapshot);
  if (!clear) {
    const lines = [
      "Widget Layout",
      `  Device: ${serial}`,
      `  Environment: ${String(snapshot.environment ?? "unknown")}`,
      `  Runtime: ${snapshot.ready === true ? "ready" : "not ready"}`,
      `  Configuration: ${String(snapshot.configurationVersion ?? "none")}`,
    ];
    if (!config) lines.push("  Layout: not configured");
    else lines.push(
      `  Grid: ${String(config.totalColumnCount ?? "?")} columns, ${String(config.totalGridCount ?? "?")} cells`,
      ...formatWidgetGroup("Permanent Widgets", config.permanentModules),
      ...formatWidgetGroup("Dynamic Widget Placements", config.overlayModules),
    );
    process.stdout.write(`${lines.join("\n")}\n`);
    return;
  }
  if (!config || (!(config.permanentModules?.length) && !(config.overlayModules?.length))) {
    process.stdout.write(`✔ Widget layout is already empty\n  Device: ${serial}\n`);
    return;
  }
  config.permanentModules = [];
  config.overlayModules = [];
  const result = await runStep("Clearing Widget layout", "Widget layout cleared", () => applyWidgetConfig(serial, config));
  process.stdout.write(`  Device: ${serial}\n  Configuration: ${String(result.configurationVersion ?? "updated")}\n`);
}

export type WidgetLaunchOptions = {
  position?: string;
  params?: string;
  paramsFile?: string;
  serial?: string;
};

export async function cmdLaunchWidget(inputValue: string, widgetPath: string, options: WidgetLaunchOptions) {
  const resolved = resolveDefinition(inputValue);
  const agentId = validateDefinition(resolved.definition);
  const [columns, rows] = widgetFamilySize(resolved.app, widgetPath);
  const params = parseOpenParams(options.params, options.paramsFile);
  const serial = await selectDevice(options.serial);
  const snapshot = await runStep("Reading Widget layout", undefined, () => readWidgetSnapshot(serial));
  if (snapshot.environment !== "dev") throw new Error("launch-widget requires Developer Mode; run `aix device set-dev`.");
  const config = readWidgetConfig(snapshot) ?? emptyWidgetConfig();
  const position = options.position === undefined ? undefined : Number(options.position);
  if (position !== undefined && (!Number.isInteger(position) || position < 0)) throw new Error("--position must be a non-negative integer.");
  const changed = upsertOverlayWidget(config, agentId, widgetPath, columns, rows, position);
  if (changed) {
    await runStep("Applying Widget layout", "Widget layout applied", () => applyWidgetConfig(serial, config));
  }
  const request = { agentId, path: widgetPath, target: "_widget", params };
  let openResult = await runStep("Launching Widget", "Widget launch requested", () => performOpenRequest(serial, request), (value) => {
    if (!changed && value.errorCode === "WIDGET_PLACEMENT_MISSING") return false;
    requireOk(value, "open");
    return true;
  });
  if (!changed && openResult.errorCode === "WIDGET_PLACEMENT_MISSING") {
    await runStep("Restoring Widget placement", "Widget placement restored", () => applyWidgetConfig(serial, config));
    openResult = await runStep("Retrying Widget launch", "Widget launch requested", () => performOpenRequest(serial, request), (value) => {
      requireOk(value, "open");
      return true;
    });
  }
  finishOpenRequest(serial, request, openResult, false);
}

function widgetFamilySize(app: Definition, widgetPath: string): [number, number] {
  const widget = Array.isArray(app.widgets)
    ? app.widgets.find((value) => value && typeof value === "object" && !Array.isArray(value) && (value as Definition).path === widgetPath) as Definition | undefined
    : undefined;
  if (!widget || typeof widget.family !== "string") {
    throw new Error(`Widget path or family is not declared in app.json: ${widgetPath}`);
  }
  const match = /^(\d+)x(\d+)$/.exec(widget.family);
  if (!match) throw new Error("Widget family must use the <rows>x<columns> format.");
  const rows = Number(match[1]);
  const columns = Number(match[2]);
  if (!rows || !columns) throw new Error("Widget family dimensions must be greater than zero.");
  return [columns, rows];
}

function moduleCells(module: Definition, totalColumns: number): number[] {
  const start = Number(module.startIndex);
  const columns = Number(module.columnSize);
  const rows = Number(module.rowSize);
  if (![start, columns, rows].every(Number.isInteger) || start < 0 || columns < 1 || rows < 1 || start % totalColumns + columns > totalColumns) {
    throw new Error("Widget module does not fit the configured grid.");
  }
  return Array.from({ length: rows }, (_, row) =>
    Array.from({ length: columns }, (_, column) => start + row * totalColumns + column),
  ).flat();
}

function upsertOverlayWidget(config: WidgetConfig, agentId: string, widgetPath: string, columns: number, rows: number, requestedPosition: number | undefined): boolean {
  const original = JSON.stringify(config);
  const totalCells = Number(config.totalGridCount ?? 4);
  const totalColumns = Number(config.totalColumnCount ?? 2);
  if (!Number.isInteger(totalCells) || !Number.isInteger(totalColumns) || totalCells < 1 || totalColumns < 1) throw new Error("Widget grid dimensions must be positive integers.");
  const permanent = (config.permanentModules ?? []) as Definition[];
  let overlays = (config.overlayModules ?? []) as Definition[];
  if (permanent.some((module) => module.agentId === agentId && module.path === widgetPath)) {
    throw new Error("Widget is already configured as permanent and cannot be launched dynamically.");
  }
  const existing = overlays.find((module) => module.agentId === agentId && module.path === widgetPath);
  overlays = overlays.filter((module) => !(module.agentId === agentId && module.path === widgetPath));
  const candidateCells = (position: number) => moduleCells({ startIndex: position, columnSize: columns, rowSize: rows }, totalColumns);
  const conflicts = (position: number, modules: Definition[]) => {
    let cells: number[];
    try {
      cells = candidateCells(position);
    } catch {
      return true;
    }
    return cells.some((cell) => cell >= totalCells) || modules.some((module) => moduleCells(module, totalColumns).some((cell) => cells.includes(cell)));
  };
  let position = requestedPosition ?? (typeof existing?.startIndex === "number" ? existing.startIndex : undefined);
  if (position === undefined) position = Array.from({ length: totalCells }, (_, index) => index).find((index) => !conflicts(index, [...permanent, ...overlays]));
  if (position === undefined) {
    overlays = [];
    position = Array.from({ length: totalCells }, (_, index) => index).find((index) => !conflicts(index, permanent));
  }
  if (position === undefined) throw new Error("No position can fit this Widget without removing a permanent Widget.");
  if (conflicts(position, permanent)) throw new Error("Requested Widget position conflicts with a permanent Widget.");
  if (conflicts(position, overlays)) {
    const cells = candidateCells(position);
    overlays = overlays.filter((module) => !moduleCells(module, totalColumns).some((cell) => cells.includes(cell)));
  }
  const usedNames = new Set([...permanent, ...overlays].map((module) => module.name).filter((name): name is string => typeof name === "string"));
  let moduleName = typeof existing?.name === "string" && /^custom_\d+$/.test(existing.name) && !usedNames.has(existing.name) ? existing.name : undefined;
  if (!moduleName) {
    for (let index = 1; ; index += 1) {
      const candidate = `custom_${index}`;
      if (!usedNames.has(candidate)) {
        moduleName = candidate;
        break;
      }
    }
  }
  overlays.push({
    name: moduleName,
    startIndex: position,
    columnSize: columns,
    rowSize: rows,
    agentId,
    path: widgetPath,
  });
  config.permanentModules = permanent;
  config.overlayModules = overlays;
  return JSON.stringify(config) !== original;
}

function resolveOpenRoute(app: Definition, requestedPath?: string, requestedTarget?: string) {
  const pages = Array.isArray(app.pages)
    ? app.pages.filter((value): value is string => typeof value === "string")
    : [];
  const widgets = Array.isArray(app.widgets)
    ? app.widgets.flatMap((widget) => {
        if (!widget || typeof widget !== "object" || Array.isArray(widget)) return [];
        const widgetPath = (widget as Record<string, unknown>).path;
        return typeof widgetPath === "string" ? [widgetPath] : [];
      })
    : [];
  const launchPath = requestedPath ?? pages[0];
  if (!launchPath) throw new Error("No launch path was provided and app.json declares no pages.");
  const isPage = pages.includes(launchPath);
  const isWidget = widgets.includes(launchPath);
  if (!isPage && !isWidget) {
    throw new Error(`Launch path is not declared in app.json pages or widgets: ${launchPath}`);
  }
  const target = requestedTarget ?? (isWidget ? "widget" : "blank");
  if (target === "widget" && !isWidget) {
    throw new Error("Target widget requires a path declared in app.json widgets.");
  }
  if ((target === "blank" || target === "current") && !isPage) {
    throw new Error(`Target ${target} requires a path declared in app.json pages.`);
  }
  if (!['blank', 'current', 'widget'].includes(target)) {
    throw new Error("Target must be blank, current, or widget.");
  }
  return { path: launchPath, target: `_${target}` };
}

function parseOpenParams(inline?: string, paramsFile?: string): Record<string, unknown> {
  if (inline && paramsFile) throw new Error("--params cannot be used with --params-file.");
  const value: unknown = inline
    ? JSON.parse(inline)
    : paramsFile
      ? JSON.parse(fs.readFileSync(paramsFile, "utf8"))
      : {};
  if (!value || typeof value !== "object" || Array.isArray(value)) {
    throw new Error("Launch params must be a JSON object.");
  }
  return value as Record<string, unknown>;
}

export async function cmdInstall(projectInput: string, options: InstallOptions) {
  const { input, isProject, definition } = resolveDefinition(projectInput, options.definition);
  const agentId = validateDefinition(definition);
  const serial = await selectDevice(options.serial);
  const temporary = fs.mkdtempSync(path.join(os.tmpdir(), "aix-install-"));
  const packagePath = path.join(temporary, "package.aix");
  const stagedDefinition = path.join(temporary, "agent.json");

  try {
    fs.writeFileSync(stagedDefinition, `${JSON.stringify(definition, null, 2)}\n`);
    if (isProject) await cmdPack(buildPackArgs(input, packagePath, options));
    else fs.copyFileSync(input, packagePath);
    const packageBytes = fs.statSync(packagePath).size;
    if (packageBytes > MAX_PACKAGE_BYTES) {
      throw new Error(`Package is larger than 128 MiB (${packageBytes} bytes).`);
    }

    const inbox = await runStep("Preparing device", undefined, () => prepareInbox(serial));
    const inboxBase = inbox.replace(/\/$/, "");
    const agentName = typeof definition.agentName === "string" && definition.agentName ? definition.agentName : agentId;
    await runStep("Uploading Agent package", undefined, async () => {
      await runAdb(serial, ["shell", "rm", "-f", `${inboxBase}/package.aix`, `${inboxBase}/agent.json`]);
      await runAdb(serial, ["push", packagePath, stagedDefinition, inbox]);
    });
    const applied = await runStep("Applying Agent package", undefined, async () => {
      const value = parseDevelopResult(await runAdb(serial, contentCall("apply")));
      requireEmptyErrorCode(value, "apply");
      if (value.agentId !== agentId) throw new Error("Apply agentId does not match agent.json.");
      if (!["CREATED", "UPDATED", "UNCHANGED", "REPAIRED"].includes(String(value.outcome))) {
        throw new Error(`Unexpected Apply outcome: ${String(value.outcome ?? "missing")}.`);
      }
      if (typeof value.operationId !== "string" || !value.operationId.trim()) throw new Error("Apply operationId is missing.");
      return value;
    });
    const status = await runStep("Checking phone upload", `Agent installed: ${agentName}`, async () => {
      const value = parseDevelopResult(await runAdb(serial, contentCall("status", applied.operationId as string)));
      requireEmptyErrorCode(value, "status");
      const state = String(value.publishState ?? "UNKNOWN");
      if (["FAILED_RETRYABLE", "FAILED_PERMANENT"].includes(state)) throw new Error(`Upload stopped with publishState=${state}.`);
      return value;
    });
    const publishState = String(status.publishState ?? "UNKNOWN");
    process.stdout.write([
      `  ID: ${agentId}`,
      `  Device: ${serial}`,
      `  Result: ${String(applied.outcome).toLowerCase()}`,
      `  Phone upload: ${publishState === "UPLOADED" ? "confirmed" : publishState.toLowerCase()}`,
      "  Cloud indexing: not verified",
    ].join("\n") + "\n");
  } finally {
    fs.rmSync(temporary, { recursive: true, force: true });
  }
}

function readLaunchMetadata(input: string, isProject: boolean): {
  app: Definition;
  agentId: string;
  agentsText?: string;
} {
  if (isProject) {
    const app = JSON.parse(fs.readFileSync(path.join(input, "app.json"), "utf8")) as Definition;
    const stateDirectory = path.join(input, ".aix");
    const idPath = path.join(stateDirectory, "agent-id");
    let id: string;
    if (fs.existsSync(idPath)) {
      id = fs.readFileSync(idPath, "utf8").trim();
      if (!id) throw new Error(`${idPath} is empty.`);
    } else {
      fs.mkdirSync(stateDirectory, { recursive: true });
      id = crypto.randomUUID();
      fs.writeFileSync(idPath, `${id}\n`, { flag: "wx" });
    }
    const agentsPath = path.join(input, "AGENTS.md");
    return {
      app,
      agentId: formatAgentId(id),
      agentsText: fs.existsSync(agentsPath) ? fs.readFileSync(agentsPath, "utf8") : undefined,
    };
  }
  const engine = loadEngine();
  const reader = new engine.AixReaderWasm(new Uint8Array(fs.readFileSync(input)));
  const version = new TextDecoder().decode(reader.read_file("VERSION")).trim();
  const app = JSON.parse(new TextDecoder().decode(reader.read_file("app.json"))) as Definition;
  let agentsText: string | undefined;
  try { agentsText = new TextDecoder().decode(reader.read_file("AGENTS.md")); } catch { /* optional */ }
  return { app, agentId: formatAgentId(version), agentsText };
}

function formatAgentId(id: string): string {
  const normalized = id.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");
  if (!normalized) throw new Error("AIX version/agent ID cannot be normalized.");
  return `develop.rokid.agent.${normalized}`;
}

function buildDefinition(app: Definition, agentId: string, agentsText?: string): Definition {
  const name = stringField(app, "agentName", "name") ?? "AIX Agent";
  const description = stringField(app, "agentDesc", "description") ?? firstParagraph(agentsText) ?? name;
  return {
    schemaVersion: 1,
    agentId,
    agentName: name,
    agentDesc: description,
    agentPrompt: stringField(app, "agentPrompt") ?? "",
    agentLogo: stringField(app, "agentLogo", "logo", "icon") ?? "",
    nativeVersion: stringField(app, "nativeVersion", "version") ?? "1.0.0",
    inkVersion: stringField(app, "inkVersion", "engine") ?? ">=0.17.0",
    permissions: Array.isArray(app.permissions) ? app.permissions : [],
  };
}

function stringField(value: Definition, ...keys: string[]): string | undefined {
  for (const key of keys) if (typeof value[key] === "string" && value[key]) return value[key] as string;
  return undefined;
}

function firstParagraph(markdown?: string): string | undefined {
  return markdown?.split(/\n\s*\n/).map((part) => part.trim())
    .find((part) => part && !part.startsWith("#") && !part.startsWith("<"));
}

function buildPackArgs(project: string, output: string, options: InstallOptions): string[] {
  const args = [project, "-o", output, "--opt-level", options.optLevel];
  if (options.optimize) args.push("--optimize");
  if (options.engine) args.push("--engine", options.engine);
  return args;
}

function validateDefinition(definition: Definition): string {
  if (!definition || typeof definition !== "object" || Array.isArray(definition)) {
    throw new Error("agent.json must contain an object.");
  }
  if (typeof definition.agentId !== "string" || !/^develop\.rokid\.[^.]+\.[^.]+(?:\..+)?$/.test(definition.agentId)) {
    throw new Error("agent.json agentId must use the develop.rokid.<business>.<id> format.");
  }
  return definition.agentId;
}

async function selectDevice(requested?: string): Promise<string> {
  const output = await runCommand("adb", ["devices"]);
  const online = output
    .split(/\r?\n/)
    .map((line) => line.trim().split(/\s+/))
    .filter((parts) => parts[1] === "device")
    .map((parts) => parts[0]);
  if (requested) {
    if (!online.includes(requested)) throw new Error(`ADB device is not online or authorized: ${requested}`);
    return requested;
  }
  if (online.length === 0) throw new Error("No online, authorized ADB device found.");
  if (online.length === 1) return online[0];
  if (!process.stdin.isTTY || !process.stdout.isTTY) {
    throw new Error("Multiple ADB devices found; pass --serial <serial> in a non-interactive shell.");
  }
  return select<string>({
    message: "Select a Rokid Glasses device",
    choices: online.map((serial) => ({ name: serial, value: serial })),
  });
}

function contentCall(method: string, arg?: string): string[] {
  const args = ["shell", "content", "call", "--uri", DEVELOP_URI, "--method", method];
  if (arg) args.push("--arg", arg);
  return args;
}

function runAdb(serial: string, args: string[]): Promise<string> {
  return runCommand("adb", ["-s", serial, ...args]);
}

function runOpenContentCall(serial: string, requestJson: string): Promise<string> {
  const quoted = `'${requestJson.split("'").join(`'"'"'`)}'`;
  return runAdb(serial, [
    "shell",
    `content call --uri ${DEVELOP_URI} --method open --arg ${quoted}`,
  ]);
}

function runCommand(command: string, args: string[]): Promise<string> {
  return new Promise((resolve, reject) => {
    const child = spawn(command, args, { stdio: ["ignore", "pipe", "pipe"] });
    const stdout: Buffer[] = [];
    const stderr: Buffer[] = [];
    child.stdout.on("data", (chunk: Buffer) => stdout.push(chunk));
    child.stderr.on("data", (chunk: Buffer) => stderr.push(chunk));
    child.on("error", reject);
    child.on("close", (code) => {
      const output = Buffer.concat(stdout).toString("utf8");
      const errorOutput = Buffer.concat(stderr).toString("utf8");
      if (code !== 0) reject(new Error(`${command} ${args.join(" ")} failed: ${(errorOutput || output).trim()}`));
      else resolve(output);
    });
  });
}

export function parseDevelopResult(text: string): DevelopResult {
  const trimmed = text.trim();
  if (trimmed.startsWith("{")) return JSON.parse(trimmed) as DevelopResult;
  const matches = [...text.matchAll(/result_data\s*=/g)];
  if (matches.length !== 1) throw new Error("Expected exactly one result_data JSON value.");
  const start = (matches[0].index ?? 0) + matches[0][0].length;
  const tail = text.slice(start).trimStart();
  if (!tail.startsWith("{")) throw new Error("result_data is not a JSON object.");
  let depth = 0;
  let quoted = false;
  let escaped = false;
  for (let index = 0; index < tail.length; index++) {
    const character = tail[index];
    if (quoted) {
      if (escaped) escaped = false;
      else if (character === "\\") escaped = true;
      else if (character === '"') quoted = false;
    } else if (character === '"') quoted = true;
    else if (character === "{") depth++;
    else if (character === "}" && --depth === 0) {
      return JSON.parse(tail.slice(0, index + 1)) as DevelopResult;
    }
  }
  throw new Error("Incomplete result_data JSON.");
}

function requireEmptyErrorCode(result: DevelopResult, stage: string) {
  if (result.errorCode !== "") {
    const message = typeof result.message === "string" && result.message
      ? `: ${result.message}`
      : typeof result.errorMessage === "string" && result.errorMessage
        ? `: ${result.errorMessage}`
        : "";
    throw new Error(`${stage} failed (${String(result.errorCode ?? "unknown error")})${message}`);
  }
}

function requireOk(result: DevelopResult, stage: string) {
  requireEmptyErrorCode(result, stage);
  if (result.ok !== true) throw new Error(`${stage} did not report ok=true.`);
}
