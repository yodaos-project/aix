import fs from "node:fs";
import path from "node:path";

function normalizeAgentId(id: string): string {
  const normalized = id.toLowerCase().replace(/[^a-z0-9_-]+/g, "-").replace(/^-+|-+$/g, "");
  if (!normalized) throw new Error("AIX version/agent ID cannot be normalized.");
  return normalized;
}

/** Shared by pack and directory commands so artifacts retain project identity. */
export function readProjectAgentId(project: string): string {
  const stateDirectory = path.join(project, ".aix");
  const idPath = path.join(stateDirectory, "agent-id");
  if (!fs.existsSync(idPath)) {
    fs.mkdirSync(stateDirectory, { recursive: true });
    try {
      fs.writeFileSync(idPath, `${crypto.randomUUID()}\n`, { flag: "wx" });
    } catch (error) {
      // Another pack/install may have initialized this project concurrently.
      if ((error as NodeJS.ErrnoException).code !== "EEXIST") throw error;
    }
  }
  const id = fs.readFileSync(idPath, "utf8").trim();
  if (!id) throw new Error(`${idPath} is empty.`);
  return normalizeAgentId(id);
}

export function formatAgentId(id: string): string {
  return `develop.rokid.agent.${normalizeAgentId(id)}`;
}
