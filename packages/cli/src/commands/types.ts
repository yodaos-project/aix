import { select } from "@inquirer/prompts";
import { clearTypeSelection, listTypeVersions, loadTypeEnvironment, readTypeSelection, saveTypeSelection, TYPE_PACKAGE } from "../services/type-environment";
import { withLoading } from "../ui/status";

export function cmdTypesCurrent(): void {
  process.stdout.write(readTypeSelection() ?? "No global type version selected; checks use project declarations");
  process.stdout.write("\n");
}

export async function cmdTypesVersions(): Promise<void> {
  const info = await withLoading("Loading type versions...", listTypeVersions);
  const selected = readTypeSelection();
  for (const version of info.versions) {
    const labels = Object.entries(info.tags).filter(([, value]) => value === version).map(([tag]) => tag);
    if (version === selected) labels.push("selected");
    process.stdout.write(`${version}${labels.length ? ` (${labels.join(", ")})` : ""}\n`);
  }
}

export async function cmdTypesSelect(version?: string): Promise<void> {
  if (!version) {
    const info = await withLoading("Loading type versions...", listTypeVersions);
    version = await select({ message: `Select ${TYPE_PACKAGE} for source checks`, pageSize: 12,
      choices: info.versions.map((value) => ({ value, name: value })),
    });
  }
  const environment = await withLoading(`Loading ${TYPE_PACKAGE}@${version}...`, () => loadTypeEnvironment(version!));
  saveTypeSelection(environment.version);
  process.stdout.write(`${environment.version}\n`);
}

export function cmdTypesReset(): void {
  clearTypeSelection();
  cmdTypesCurrent();
}
