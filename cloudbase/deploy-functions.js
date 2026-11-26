const { spawnSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const root = __dirname;
const localConfigPath = path.join(root, "cloudbaserc.json");
const exampleConfigPath = path.join(root, "cloudbaserc.example.json");
const configPath = fs.existsSync(localConfigPath) ? localConfigPath : exampleConfigPath;
const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
const envId = String(process.env.CLOUDBASE_ENV_ID || config.envId || "").trim();
const requested = process.argv.slice(2).map((item) => String(item || "").trim()).filter(Boolean);
const configured = (config.functions || []).map((item) => String(item.name || "").trim()).filter(Boolean);
const names = requested.length ? requested : configured;
const npmCommand = "npm";
const tcbCommand = "tcb";

if (!envId || envId.startsWith("replace-")) {
  console.error("Set CLOUDBASE_ENV_ID or create cloudbase/cloudbaserc.json first.");
  process.exit(1);
}

for (const name of names) {
  if (!/^csw_[a-z0-9_]+$/i.test(name)) {
    console.error(`Refusing to deploy a function without the csw_ prefix: ${name}`);
    process.exit(1);
  }
  const functionPath = path.join(root, "functions", name);
  if (!fs.existsSync(path.join(functionPath, "index.js"))) {
    console.error(`Function folder is incomplete: ${functionPath}`);
    process.exit(1);
  }
  if (fs.existsSync(path.join(functionPath, "package.json"))) {
    console.log(`Installing production dependencies for ${name}`);
    const install = spawnSync(npmCommand, ["install", "--omit=dev"], {
      cwd: functionPath,
      stdio: "inherit",
      shell: process.platform === "win32"
    });
    if (install.status !== 0) process.exit(install.status || 1);
  }
  console.log(`Deploying ${name} to ${envId}`);
  const result = spawnSync(tcbCommand, ["fn", "deploy", name, "-e", envId, "--force", "--yes"], {
    // Run from the CloudBase root so the CLI loads cloudbaserc.json,
    // including functionRoot and per-function environment variables.
    cwd: root,
    stdio: "inherit",
    shell: process.platform === "win32"
  });
  if (result.status !== 0) process.exit(result.status || 1);
}
