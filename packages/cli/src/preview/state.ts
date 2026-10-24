import fs from "node:fs";
import path from "node:path";
import { loadEngine } from "../wasm";
import { walkDirectory } from "../walk";
import type { PreviewState } from "./types";
import { sanitizeAppId } from "./utils";

export function buildPreviewState(inputPath: string): PreviewState {
  const resolvedPath = path.resolve(inputPath);
  const stat = fs.statSync(resolvedPath);
  if (stat.isDirectory()) {
    return buildPreviewStateFromDirectory(resolvedPath);
  }
  return buildPreviewStateFromAixFile(resolvedPath);
}

function buildPreviewStateFromAixFile(filePath: string): PreviewState {
  const data = fs.readFileSync(filePath);
  const engineApi = loadEngine();
  const reader = new engineApi.AixReaderWasm(new Uint8Array(data));
  return {
    appId: sanitizeAppId(path.basename(filePath).replace(/\.aix$/i, "")),
    sourceName: path.basename(filePath),
    sourceKind: "aix-file",
    title: reader.get_title(),
    version: reader.get_version(),
    files: reader.list().map((entry) => ({
      path: entry.name,
      base64: Buffer.from(reader.read_file(entry.name)).toString("base64"),
    })),
  };
}

function buildPreviewStateFromDirectory(dirPath: string): PreviewState {
  const files = loadEngine().collect_aix_source(walkDirectory(dirPath));
  const appJsonFile = files.find((file) => file.path === "app.json");
  const versionFile = files.find((file) => file.path === "VERSION");
  let title: string | undefined;
  if (appJsonFile) {
    try {
      const appJson = JSON.parse(Buffer.from(appJsonFile.data).toString("utf-8"));
      title = appJson?.window?.navigationBarTitleText ?? appJson?.title;
    } catch {
      title = undefined;
    }
  }

  const version = versionFile
    ? Buffer.from(versionFile.data).toString("utf-8").trim() || undefined
    : undefined;

  return {
    appId: sanitizeAppId(path.basename(dirPath)),
    sourceName: path.basename(dirPath),
    sourceKind: "directory",
    title,
    version,
    files: files.map((file) => ({
      path: file.path,
      base64: Buffer.from(file.data).toString("base64"),
    })),
  };
}
