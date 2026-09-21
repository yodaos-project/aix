import { Argument, Command } from "commander";
import { cmdOptimize, cmdList, cmdPack } from "./commands/legacy";
import { cmdRuntimeCurrent } from "./commands/runtime/current";
import { cmdRuntimeSelect } from "./commands/runtime/select";
import { cmdRuntimeVersions } from "./commands/runtime/versions";
import { cmdPreview } from "./preview";
import {
  cmdDevice,
  cmdInstall,
  cmdLaunchPage,
  cmdLaunchWidget,
  cmdWidgetLayout,
} from "./commands/launch";
import { cmdShow } from "./commands/show";
import { formatError } from "./ui/status";
import { confirm } from "@inquirer/prompts";

const PACKAGE_COMMANDS = "Package Commands:";
const DEVICE_COMMANDS = "Device Commands:";
const PREVIEW_COMMANDS = "Preview Commands:";

async function main() {
  const program = new Command();
  program
    .name("aix")
    .description("AIX package manager")
    .commandsGroup("Other Commands:")
    .helpCommand(true)
    .showHelpAfterError();

  program
    .command("pack <input-dir>")
    .helpGroup(PACKAGE_COMMANDS)
    .description("Pack a directory into an .aix artifact")
    .option("-o, --output <output>", "Output file")
    .option("-O, --optimize", "Enable optimization")
    .option("--opt-level <level>", "Optimization level, 1-3", "2")
    .option("--engine <range>", "Supported engine range")
    .option("--log-time", "Prefix pack log lines with a local timestamp")
    .action(async (inputDir: string, options: {
      output?: string;
      optimize?: boolean;
      optLevel: string;
      engine?: string;
      logTime?: boolean;
    }) => {
      await cmdPack(buildPackArgs(inputDir, options));
    });

  program
    .command("list <aix-file>")
    .helpGroup(PACKAGE_COMMANDS)
    .alias("ls")
    .description("List files inside an .aix artifact")
    .action((aixFile: string) => {
      cmdList([aixFile]);
    });

  program
    .command("optimize <aix-file>")
    .helpGroup(PACKAGE_COMMANDS)
    .description("Optimize an existing .aix artifact")
    .requiredOption("-o, --output <output>", "Output file")
    .option("--level <level>", "Optimization level, 1-3", "2")
    .action((aixFile: string, options: { output: string; level: string }) => {
      cmdOptimize(buildOptimizeArgs(aixFile, options));
    });

  program
    .command("show <input>")
    .helpGroup(PACKAGE_COMMANDS)
    .description("Show the effective Agent Definition JSON")
    .option("--definition <file>", "Definition JSON override")
    .option("-o, --output <file>", "Write JSON to a file")
    .option("--compact", "Print compact single-line JSON")
    .action((input: string, options: {
      definition?: string;
      output?: string;
      compact?: boolean;
    }) => {
      cmdShow(input, options);
    });

  program
    .command("install <input>")
    .helpGroup(DEVICE_COMMANDS)
    .description("Install an AIX Agent on Rokid Glasses over ADB")
    .option("--definition <file>", "Definition JSON (default: <project>/agent.json)")
    .option("-s, --serial <serial>", "ADB device serial")
    .option("-O, --optimize", "Enable optimization")
    .option("--opt-level <level>", "Optimization level, 1-3", "2")
    .option("--engine <range>", "Supported engine range")
    .action(async (input: string, options) => {
      await cmdInstall(input, options);
    });

  program
    .command("device")
    .helpGroup(DEVICE_COMMANDS)
    .description("Inspect the device or set/unset Developer Mode")
    .addArgument(new Argument("[action]", "Developer Mode action").choices(["set-dev", "unset-dev"]))
    .option("-s, --serial <serial>", "ADB device serial")
    .action(async (action: string | undefined, options: { serial?: string }) => {
      await cmdDevice(action, options.serial);
    });

  program
    .command("launch-page <input> [path]")
    .helpGroup(DEVICE_COMMANDS)
    .description("Open an installed AIX Agent Page on Rokid Glasses")
    .option("--card", "Open as a card instead of full screen")
    .option("--params <json>", "Parameters as a JSON object")
    .option("--params-file <file>", "Read parameters from a JSON file")
    .option("-s, --serial <serial>", "ADB device serial")
    .action(async (input: string, path: string | undefined, options: {
      card?: boolean;
      params?: string;
      paramsFile?: string;
      serial?: string;
    }) => {
      await cmdLaunchPage(input, path, options);
    });

  program
    .command("launch-widget <input> <path>")
    .helpGroup(DEVICE_COMMANDS)
    .description("Configure and open an installed overlay Widget on Rokid Glasses")
    .option("-p, --position <index>", "Widget grid start index")
    .option("--params <json>", "Parameters as a JSON object")
    .option("--params-file <file>", "Read parameters from a JSON file")
    .option("-s, --serial <serial>", "ADB device serial")
    .action(async (input: string, path: string, options) => {
      await cmdLaunchWidget(input, path, options);
    });

  program
    .command("widget-layout")
    .helpGroup(DEVICE_COMMANDS)
    .description("Show or clear the current device Widget layout")
    .addArgument(new Argument("[action]", "Read-only layout action").choices(["show"]))
    .option("--clear", "Clear persistent and overlay Widget placements")
    .option("--yes", "Skip confirmation for --clear")
    .option("-s, --serial <serial>", "ADB device serial")
    .action(async (action: string | undefined, options: {
      clear?: boolean;
      yes?: boolean;
      serial?: string;
    }) => {
      if (action !== undefined && action !== "show") throw new Error("Widget layout action must be show.");
      if (action === "show" && options.clear) throw new Error("show cannot be used with --clear.");
      if (options.yes && !options.clear) throw new Error("--yes requires --clear.");
      if (options.clear && !options.yes) {
        if (!process.stdin.isTTY) throw new Error("--clear requires confirmation; rerun with --yes in a non-interactive shell.");
        const accepted = await confirm({ message: "Clear all permanent and dynamic Widget placements?", default: false });
        if (!accepted) throw new Error("Widget layout clear cancelled.");
      }
      await cmdWidgetLayout(options.clear ?? false, options.serial);
    });

  program
    .command("preview <input>")
    .helpGroup(PREVIEW_COMMANDS)
    .description("Preview an .aix artifact or source directory")
    .option("--html-out <file>", "Write the preview HTML to a file")
    .option("--dev", "Start the preview server in development mode")
    .option("--launch", "Open the preview URL in the default browser")
    .option("--launch-target <target>", "Target for --launch: blank or current", "blank")
    .action(async (input: string, options: {
      htmlOut?: string;
      dev?: boolean;
      launch?: boolean;
      launchTarget: string;
    }) => {
      await cmdPreview(buildPreviewArgs(input, options));
    });

  const runtime = program
    .command("runtime")
    .helpGroup(PREVIEW_COMMANDS)
    .description("Inspect available preview runtime versions")
    .action(() => {
      runtime.outputHelp();
    });

  runtime
    .command("versions")
    .description("List available preview runtime versions")
    .action(async () => {
      await cmdRuntimeVersions();
    });

  runtime
    .command("current")
    .description("Print the current default runtime version")
    .action(async () => {
      await cmdRuntimeCurrent();
    });

  runtime
    .command("select")
    .description("Select a runtime version interactively")
    .action(async () => {
      await cmdRuntimeSelect();
    });

  await program.parseAsync(process.argv);
}

function buildPackArgs(
  inputDir: string,
  options: {
    output?: string;
    optimize?: boolean;
    optLevel: string;
    engine?: string;
    logTime?: boolean;
  },
): string[] {
  const args = [inputDir];
  if (options.output) {
    args.push("-o", options.output);
  }
  if (options.optimize) {
    args.push("--optimize");
  }
  if (options.optLevel) {
    args.push("--opt-level", options.optLevel);
  }
  if (options.engine) {
    args.push("--engine", options.engine);
  }
  if (options.logTime) {
    args.push("--log-time");
  }
  return args;
}

function buildOptimizeArgs(
  aixFile: string,
  options: { output: string; level: string },
): string[] {
  return [aixFile, "-o", options.output, "--level", options.level];
}

function buildPreviewArgs(
  input: string,
  options: { htmlOut?: string; dev?: boolean; launch?: boolean; launchTarget: string },
): string[] {
  const args = [input];
  if (options.htmlOut) {
    args.push("--html-out", options.htmlOut);
  }
  if (options.dev) {
    args.push("--dev");
  }
  if (options.launch) {
    args.push("--launch");
  }
  args.push("--launch-target", options.launchTarget);
  return args;
}

main().catch((error: unknown) => {
  const message = error instanceof Error ? error.message : String(error);
  process.stderr.write(`${formatError(message)}\n`);
  process.exit(1);
});
