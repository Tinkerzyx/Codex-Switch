import { spawn } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { delimiter, join } from "node:path";
import { fileURLToPath } from "node:url";

// Desktop-launched terminals do not always inherit rustup's PATH entry.
const env = { ...process.env };
const rustBin = join(env.CARGO_HOME || join(homedir(), ".cargo"), "bin");
const pathKey = Object.keys(env).find((key) => key.toLowerCase() === "path") || "PATH";
env[pathKey] = [rustBin, env[pathKey]].filter(Boolean).join(delimiter);

const [tool, ...args] = process.argv.slice(2);
let command;
let commandArgs;
if (tool === "tauri") {
  command = process.execPath;
  commandArgs = [fileURLToPath(new URL("../node_modules/@tauri-apps/cli/tauri.js", import.meta.url)), ...args];
} else if (tool === "cargo") {
  const cargo = join(rustBin, process.platform === "win32" ? "cargo.exe" : "cargo");
  command = existsSync(cargo) ? cargo : "cargo";
  commandArgs = args;
} else {
  throw new Error("Expected cargo or tauri as the first argument.");
}

const child = spawn(command, commandArgs, { env, stdio: "inherit" });
child.on("error", (error) => {
  console.error(`Unable to run ${tool}: ${error.message}`);
  process.exitCode = 1;
});
child.on("exit", (code) => {
  process.exitCode = code ?? 1;
});
