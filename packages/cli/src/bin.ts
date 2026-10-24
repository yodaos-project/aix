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
    process.stderr.write("Starting source checks\n");
  }
}

// The command bundle stays separate: a static import would initialize all CLI
// commands before the startup status above, and would defeat this entry point.
require(path.join(__dirname, "cli-main.js"));
