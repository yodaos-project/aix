import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import { createHash, randomUUID } from "node:crypto";
import { Parser } from "tar";
import type { CheckOptions } from "../wasm";

export const TYPE_PACKAGE = "@yodaos-pkg/ink-env";
const PREFIX = `node_modules/${TYPE_PACKAGE}/`;
const AIX_HOME_DIR = path.join(os.homedir(), ".aix");
const SELECTION = path.join(AIX_HOME_DIR, "types.json");
const MAX_BYTES = 32 * 1024 * 1024;
const EXACT_VERSION = /^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?(?:\+[0-9A-Za-z.-]+)?$/;

type TypeFile = NonNullable<CheckOptions["typeFiles"]>[number];
export type TypeEnvironment = { version: string; files: TypeFile[] };
type VersionInfo = { version: string; dist: { tarball: string; integrity?: string; shasum?: string } };
type RegistryInfo = { "dist-tags": Record<string, string>; versions: Record<string, VersionInfo>; time?: Record<string, string> };

export function readTypeSelection(): string | undefined {
  try {
    const value = JSON.parse(fs.readFileSync(SELECTION, "utf8")).selectedVersion;
    if (typeof value !== "string" || !EXACT_VERSION.test(value)) throw new Error("expected an exact version");
    return value;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw new Error(`failed to read type selection from ~/.aix/types.json: ${String(error)}`);
  }
}

export function saveTypeSelection(version: string): void {
  if (!EXACT_VERSION.test(version)) throw new Error(`Invalid type version: ${version}`);
  atomicWrite(SELECTION, { selectedVersion: version });
}

export function clearTypeSelection(): void {
  fs.rmSync(SELECTION, { force: true });
}

export async function listTypeVersions(): Promise<{ versions: string[]; tags: Record<string, string> }> {
  const info = await registryInfo();
  return {
    versions: Object.keys(info.versions).filter((version) => EXACT_VERSION.test(version))
      .sort((a, b) => (info.time?.[b] ?? "").localeCompare(info.time?.[a] ?? "") || b.localeCompare(a, undefined, { numeric: true })),
    tags: info["dist-tags"],
  };
}

/** Resolve only an explicitly selected package; no latest-version fallback. */
export async function loadTypeEnvironment(request: string): Promise<TypeEnvironment> {
  if (!EXACT_VERSION.test(request) && !/^[A-Za-z][A-Za-z0-9_-]*$/.test(request)) {
    throw new Error("Type version must be an exact npm version or dist-tag");
  }
  if (EXACT_VERSION.test(request)) {
    const cached = readCache(request);
    if (cached) return cached;
  }
  const info = await registryInfo();
  const version = info["dist-tags"][request] ?? request;
  if (!EXACT_VERSION.test(version) || !info.versions[version]) throw new Error(`Unknown ${TYPE_PACKAGE} version: ${request}`);
  const cached = readCache(version);
  if (cached) return cached;
  const published = info.versions[version];
  const response = await fetch(published.dist.tarball, { signal: AbortSignal.timeout(30_000) });
  if (!response.ok) throw new Error(`failed to download ${TYPE_PACKAGE}@${version} (HTTP ${response.status})`);
  const archive = await readLimited(response);
  verifyIntegrity(archive, published.dist);
  const files = await declarations(archive);
  const result = { version, files };
  validateEnvironment(result, version);
  atomicWrite(cacheFile(version), result);
  return result;
}

function cacheFile(version: string): string {
  return path.join(AIX_HOME_DIR, "types", "ink-env", `${version}.json`);
}

function readCache(version: string): TypeEnvironment | undefined {
  let content: string;
  try {
    content = fs.readFileSync(cacheFile(version), "utf8");
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return undefined;
    throw error;
  }
  const environment = JSON.parse(content) as TypeEnvironment;
  validateEnvironment(environment, version);
  return environment;
}

function validateEnvironment(environment: TypeEnvironment, version: string): void {
  if (environment.version !== version || !Array.isArray(environment.files) || !environment.files.length) {
    throw new Error("Invalid cached type environment");
  }
  const names = new Set<string>();
  for (const file of environment.files) {
    if (typeof file.path !== "string" || !file.path.startsWith(PREFIX) || !safePath(file.path)
      || !/(?:\.json|\.d\.[cm]?ts)$/.test(file.path) || typeof file.source !== "string" || names.has(file.path)) {
      throw new Error("Invalid type declaration file");
    }
    names.add(file.path);
  }
  const metadata = JSON.parse(environment.files.find((file) => file.path === PREFIX + "package.json")?.source ?? "null");
  if (metadata?.name !== TYPE_PACKAGE || metadata?.version !== version
    || !environment.files.some((file) => /\.d\.[cm]?ts$/.test(file.path))) {
    throw new Error(`Invalid ${TYPE_PACKAGE}@${version} package metadata or declarations`);
  }
  if (Object.keys(metadata.dependencies ?? {}).length) {
    throw new Error(`${TYPE_PACKAGE}@${version} has dependencies; install it in the project with npm instead`);
  }
}

function safePath(name: string): boolean {
  return !/[\\\x00:]/.test(name) && name.split("/").every((part) => part && part !== "." && part !== "..");
}

async function registryInfo(): Promise<RegistryInfo> {
  const provider = process.env.AIX_NPM_REGISTRY?.trim() || "npm";
  if (provider !== "npm" && provider !== "npmmirror") throw new Error("invalid AIX_NPM_REGISTRY (expected npm or npmmirror)");
  const host = provider === "npm" ? "registry.npmjs.org" : "registry.npmmirror.com";
  const response = await fetch(`https://${host}/@yodaos-pkg%2Fink-env`, {
    headers: { Accept: "application/json" }, signal: AbortSignal.timeout(30_000),
  });
  if (!response.ok) throw new Error(`failed to fetch type versions (HTTP ${response.status})`);
  return JSON.parse((await readLimited(response)).toString("utf8")) as RegistryInfo;
}

async function readLimited(response: Response): Promise<Buffer> {
  const chunks: Uint8Array[] = [];
  let bytes = 0;
  const reader = response.body?.getReader();
  if (!reader) throw new Error("Empty type package response");
  try {
    while (true) {
      const { value, done } = await reader.read();
      if (done) break;
      bytes += value.byteLength;
      if (bytes > MAX_BYTES) throw new Error("Type package exceeds 32 MiB");
      chunks.push(value);
    }
  } finally {
    await reader.cancel();
    reader.releaseLock();
  }
  return Buffer.concat(chunks);
}

function verifyIntegrity(archive: Buffer, dist: VersionInfo["dist"]): void {
  const integrity = dist.integrity?.split(/\s+/).find((value) => /^sha512-/.test(value));
  if (integrity) {
    if (createHash("sha512").update(archive).digest("base64") === integrity.slice(7)) return;
  } else if (dist.shasum && createHash("sha1").update(archive).digest("hex") === dist.shasum) return;
  throw new Error("Type package integrity verification failed");
}

/** Read declarations in memory; never extract or execute npm package contents. */
function declarations(archive: Buffer): Promise<TypeFile[]> {
  return new Promise((resolve, reject) => {
    const files: TypeFile[] = [];
    let bytes = 0;
    const parser = new Parser({ strict: true, onReadEntry(entry) {
      bytes += entry.size;
      if (bytes > MAX_BYTES) {
        parser.abort(new Error("Type package expands beyond 32 MiB"));
        return;
      }
      if (entry.type !== "File" || !entry.path.startsWith("package/") || !safePath(entry.path)
        || !/(?:\.json|\.d\.[cm]?ts)$/.test(entry.path)) {
        entry.resume();
        return;
      }
      const chunks: Buffer[] = [];
      entry.on("data", (chunk: Buffer) => chunks.push(chunk));
      entry.on("end", () => files.push({ path: PREFIX + entry.path.slice(8), source: Buffer.concat(chunks).toString("utf8") }));
    } });
    parser.on("error", reject);
    parser.on("end", () => resolve(files.sort((a, b) => a.path.localeCompare(b.path))));
    parser.end(archive);
  });
}

function atomicWrite(file: string, value: unknown): void {
  fs.mkdirSync(path.dirname(file), { recursive: true });
  const temporary = `${file}.${process.pid}.${randomUUID()}.tmp`;
  try {
    fs.writeFileSync(temporary, JSON.stringify(value) + "\n", { flag: "wx" });
    fs.renameSync(temporary, file);
  } finally {
    fs.rmSync(temporary, { force: true });
  }
}
