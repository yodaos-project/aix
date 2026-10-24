import path from "node:path";

// Keep this entry independent of command/UI dependencies so cold module loading
// cannot delay the first visible check status. Commander still parses arguments.
const args = process.argv.slice(2);
if (args[0] === "check") {
  let format = "text";
  let help = false;
  for (let index = 1; index < args.length; index++) {
    const arg = args[index];
    if (arg === "--") break;
    if (arg === "--help" || arg === "-h") help = true;
    if (arg === "--format") format = args[++index] ?? "";
    else if (arg.startsWith("--format=")) format = arg.slice("--format=".length);
  }
  if (format === "text" && !help) {
    const text = "Starting source checks";
    process.stderr.write(process.stderr.isTTY
      ? `${text.slice(0, Math.max(1, (process.stderr.columns || 80) - 1))}\r`
      : `${text}\n`);
  }
}

// The command bundle stays separate: a static import would initialize all CLI
// commands before the startup status above, and would defeat this entry point.
require(path.join(__dirname, "cli-main.js"));
