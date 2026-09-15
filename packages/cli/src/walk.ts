/// <reference types="node" />

import fs from "fs";
import path from "path";

export type WalkedFile = { path: string; data: Uint8Array };

export function walkDirectory(dir: string): WalkedFile[] {
  const stat = fs.statSync(dir);
  if (!stat.isDirectory()) {
    throw new Error("Input path is not a directory");
  }
  const rootDir = path.resolve(dir);
  const results: WalkedFile[] = [];

  const visit = (current: string) => {
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const abs = path.join(current, entry.name);
      if (entry.isDirectory()) {
        if (entry.name === ".aix") continue;
        visit(abs);
      } else if (entry.isFile()) {
        const rel = path.relative(rootDir, abs).split(path.sep).join("/");
        results.push({ path: rel, data: new Uint8Array(fs.readFileSync(abs)) });
      }
    }
  };

  visit(rootDir);
  return results;
}
