import fs from "node:fs";
import { parseArgs } from "node:util";
import {
  loadEngine,
  AixInputFile,
  AixPackResult,
} from "../wasm";
import type { PackProgressEvent } from "../wasm";
import { walkDirectory, WalkedFile } from "../walk";
import { readProjectAgentId } from "../agent-identity";
import ora from "ora";
import { formatError } from "../ui/status";

type PackLogger = {
  writeLine(message: string): void;
};

function formatSize(bytes: number): string {
  const KB = 1024;
  const MB = KB * 1024;
  const GB = MB * 1024;
  if (bytes >= GB) return `${(bytes / GB).toFixed(2)} GB`;
  if (bytes >= MB) return `${(bytes / MB).toFixed(2)} MB`;
  if (bytes >= KB) return `${(bytes / KB).toFixed(2)} KB`;
  return `${bytes} bytes`;
}

function printError(message: string): never {
  process.stderr.write(`${formatError(message)}\n`);
  process.exit(1);
}

function readFileAsBytes(filePath: string): Uint8Array {
  return new Uint8Array(fs.readFileSync(filePath));
}

function writeOutput(data: Uint8Array, outputPath: string) {
  fs.writeFileSync(outputPath, Buffer.from(data));
}

function formatPackFileLine(
  file: AixPackResult["report"]["files"][number],
): string | null {
  const lines: string[] = [];
  if (file.converted_to_utf8) {
    lines.push(`Converted ${file.path} to UTF-8 for packaging`);
  }
  if (file.status === "optimized") {
    lines.push(
      `Optimized ${file.path}: ${formatSize(file.original_size)} -> ${formatSize(file.output_size)} (saved ${formatSize(file.saved_bytes)})`,
    );
  } else if (file.path !== "VERSION") {
    lines.push(`Adding file: ${file.path}`);
  }
  return lines.length > 0 ? lines.join("\n") : null;
}

function formatPackSummaryLine(
  report: AixPackResult["report"],
  optimize: boolean,
): string | null {
  if (!optimize || report.original_size <= 0) {
    return null;
  }
  const ratio = (report.saved_bytes / report.original_size) * 100;
  return `Optimization Summary: Total saved ${formatSize(report.saved_bytes)} (${ratio.toFixed(2)}%)`;
}

function parseLevel(value: string, flagName: string): 1 | 2 | 3 {
  const level = Number(value);
  if (!Number.isInteger(level) || level < 1 || level > 3) {
    printError(`${flagName} must be an integer between 1 and 3`);
  }
  return level as 1 | 2 | 3;
}

function formatLogTime(date: Date): string {
  const hours = String(date.getHours()).padStart(2, "0");
  const minutes = String(date.getMinutes()).padStart(2, "0");
  const seconds = String(date.getSeconds()).padStart(2, "0");
  const milliseconds = String(date.getMilliseconds()).padStart(3, "0");
  return `${hours}:${minutes}:${seconds}.${milliseconds}`;
}

function createPackLogger(logTime: boolean): PackLogger {
  return {
    writeLine(message: string) {
      for (const line of message.split("\n")) {
        if (logTime) {
          process.stdout.write(`[${formatLogTime(new Date())}] `);
        }
        process.stdout.write(`${line}\n`);
      }
    },
  };
}

export async function cmdPack(args: string[]) {
  const { values, positionals } = parseArgs({
    args,
    allowPositionals: true,
    options: {
      o: { type: "string", short: "o" },
      optimize: { type: "boolean", short: "O" },
      "opt-level": { type: "string", default: "2" },
      engine: { type: "string" },
      "log-time": { type: "boolean" },
    },
  });
  if (positionals.length !== 1) {
    printError("pack requires an input directory");
  }
  const inputDir = positionals[0];
  const outputPath = values.o ?? "bundle.aix";
  const optimize = values.optimize ?? false;
  const optLevel = parseLevel(values["opt-level"], "--opt-level");
  const engine = values.engine;
  const logTime = values["log-time"] ?? false;
  const logger = createPackLogger(logTime);

  const spinner = process.stderr.isTTY && !logTime ? ora().start() : null;
  const status = (text: string) => {
    if (spinner) {
      spinner.text = text;
    } else {
      logger.writeLine(text);
    }
  };

  try {
    status(`Scanning source files in ${inputDir}`);
    const files = walkDirectory(inputDir);
    status(`Collected ${files.length} source files`);
    status("Loading WASM engine");
    const engineApi = loadEngine();
    status("WASM engine ready");

    const optimizeArg = optimize
      ? { level: optLevel, json: true, png: true, jpeg: true }
      : null;

    const buildId = crypto.randomUUID();
    const agentId = readProjectAgentId(inputDir);
    const sourceBuilderCtor = engineApi.AixSourcePackBuilderWasm;
    const progressPack = engineApi.pack_aix_from_source_with_progress;
    const handlePackProgress = (event: PackProgressEvent) => {
      if (event.type === "transferring_files_to_wasm") {
        status("Transferring files to WASM");
        return;
      }
      if (event.type === "collecting_source_inputs") {
        status("Collecting source inputs");
        return;
      }
      if (event.type === "resolving_engine") {
        status("Resolving engine range");
        return;
      }
      if (event.type === "preparing_files") {
        status("Preparing files for packaging");
        return;
      }
      if (event.type === "finalizing_archive") {
        status("Finalizing package archive");
        return;
      }
      if (event.type !== "file_finished") {
        return;
      }
      const line = formatPackFileLine(
        event.report as AixPackResult["report"]["files"][number],
      );
      if (line) {
        logger.writeLine(line);
      }
    };
    if (sourceBuilderCtor) {
      status("Creating WASM source builder");
    } else if (progressPack) {
      status("Invoking WASM packer");
    }
    let result: AixPackResult;
    if (sourceBuilderCtor) {
      const sourceBuilder = new sourceBuilderCtor();
      status("Transferring files to WASM incrementally");
      for (const file of files) {
        sourceBuilder.add_file(file.path, file.data);
      }
      status("Finished transferring files to WASM");
      result = sourceBuilder.pack_from_source_with_progress(
        buildId,
        engine,
        optimizeArg,
        handlePackProgress,
        agentId,
      );
    } else {
      const inputFiles: AixInputFile[] = files.map((f: WalkedFile) => ({
        path: f.path,
        data: f.data,
      }));
      result = progressPack
        ? progressPack(inputFiles, buildId, engine, optimizeArg, handlePackProgress, agentId)
        : engineApi.pack_aix_from_source(inputFiles, buildId, engine, optimizeArg, agentId);
    }

    for (const warning of result.warnings ?? []) {
      process.stderr.write(`warning: ${warning}\n`);
    }

    writeOutput(result.data, outputPath);

    if (!sourceBuilderCtor && !progressPack) {
      for (const file of result.report.files) {
        const line = formatPackFileLine(file);
        if (line) {
          logger.writeLine(line);
        }
      }
    }

    const finalSize = fs.statSync(outputPath).size;
    if (spinner) {
      spinner.succeed(`Package created: ${outputPath} (${formatSize(finalSize)})`);
    } else {
      logger.writeLine(`Package created: ${outputPath} (${formatSize(finalSize)})`);
    }
    const summaryLine = formatPackSummaryLine(result.report, optimize);
    if (summaryLine) {
      logger.writeLine(summaryLine);
    }
  } catch (error) {
    spinner?.fail();
    throw error;
  }
}

export function cmdList(args: string[]) {
  const { positionals } = parseArgs({ args, allowPositionals: true });
  if (positionals.length !== 1) {
    printError("list requires an .aix file");
  }
  const filePath = positionals[0];
  const engineApi = loadEngine();
  const reader = new engineApi.AixReaderWasm(readFileAsBytes(filePath));

  process.stdout.write(`Contents of ${filePath}:\n`);
  for (const entry of reader.list()) {
    process.stdout.write(
      `${entry.name}: ${formatSize(entry.size)} (compressed: ${formatSize(entry.compressed_size)})\n`,
    );
  }
}

export function cmdOptimize(args: string[]) {
  const { values, positionals } = parseArgs({
    args,
    allowPositionals: true,
    options: {
      o: { type: "string", short: "o" },
      level: { type: "string", default: "2" },
    },
  });
  if (positionals.length !== 1) {
    printError("optimize requires an .aix file");
  }
  if (!values.o) {
    printError("optimize requires an output path (-o)");
  }
  const inputPath = positionals[0];
  const outputPath = values.o;
  const level = parseLevel(values.level, "--level");

  const engineApi = loadEngine();
  const result = engineApi.optimize_aix(readFileAsBytes(inputPath), {
    level,
    json: true,
    png: true,
    jpeg: true,
  });

  writeOutput(result.data, outputPath);
  process.stdout.write(
    `Optimized ${inputPath} to ${outputPath} (saved ${formatSize(result.report.saved_bytes)})\n`,
  );
}
