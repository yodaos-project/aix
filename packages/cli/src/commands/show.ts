import fs from "node:fs";
import { resolveDefinition } from "./launch";

export type ShowOptions = {
  definition?: string;
  output?: string;
  compact?: boolean;
};

export function cmdShow(input: string, options: ShowOptions) {
  const definition = resolveDefinition(input, options.definition).definition;
  const json = `${JSON.stringify(definition, null, options.compact ? undefined : 2)}\n`;
  if (options.output) fs.writeFileSync(options.output, json);
  else process.stdout.write(json);
}
