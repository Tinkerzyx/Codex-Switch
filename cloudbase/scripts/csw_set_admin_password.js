const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

function promptHidden(label) {
  return new Promise((resolve, reject) => {
    if (!process.stdin.isTTY || typeof process.stdin.setRawMode !== "function") {
      reject(new Error("An interactive terminal is required."));
      return;
    }
    let value = "";
    process.stdout.write(label);
    process.stdin.setEncoding("utf8");
    process.stdin.setRawMode(true);
    process.stdin.resume();

    const cleanup = () => {
      process.stdin.off("data", onData);
      process.stdin.setRawMode(false);
      process.stdin.pause();
      process.stdout.write("\n");
    };
    const onData = (chunk) => {
      for (const char of chunk) {
        if (char === "\u0003") {
          cleanup();
          reject(new Error("Password update cancelled."));
          return;
        }
        if (char === "\r" || char === "\n") {
          cleanup();
          resolve(value);
          return;
        }
        if (char === "\u007f" || char === "\b") {
          value = value.slice(0, -1);
          continue;
        }
        if (char >= " ") value += char;
      }
    };
    process.stdin.on("data", onData);
  });
}

function parseLocalEnv(raw) {
  const values = new Map();
  for (const line of raw.split(/\r?\n/)) {
    if (!line || line.trimStart().startsWith("#")) continue;
    const separator = line.indexOf("=");
    if (separator > 0) values.set(line.slice(0, separator), line.slice(separator + 1));
  }
  return values;
}

async function main() {
  const supplied = process.argv.slice(2).join(" ").trim();
  const password = supplied || await promptHidden("New administrator password: ");
  if (password.length < 10) throw new Error("Administrator password must contain at least 10 characters.");
  if (!supplied) {
    const confirmation = await promptHidden("Confirm administrator password: ");
    if (confirmation !== password) throw new Error("The two passwords do not match.");
  }

  const root = path.resolve(__dirname, "..");
  const configPath = path.join(root, "cloudbaserc.json");
  const envPath = path.join(root, ".env.local");
  if (!fs.existsSync(configPath) || !fs.existsSync(envPath)) {
    throw new Error("Run the initial secrets setup before changing the administrator password.");
  }

  const localValues = parseLocalEnv(fs.readFileSync(envPath, "utf8"));
  const salt = crypto.randomBytes(16).toString("base64");
  const passwordHash = crypto.scryptSync(password, Buffer.from(salt, "base64"), 64).toString("base64");
  localValues.set("CSW_ADMIN_PASSWORD", password);
  localValues.set("CSW_ADMIN_PASSWORD_SALT", salt);
  localValues.set("CSW_ADMIN_PASSWORD_HASH", passwordHash);

  const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  const adminFunction = (config.functions || []).find((item) => item.name === "csw_admin_api");
  if (!adminFunction) throw new Error("csw_admin_api is missing from cloudbaserc.json.");
  adminFunction.envVariables = {
    ...(adminFunction.envVariables || {}),
    CSW_ADMIN_PASSWORD_SALT: salt,
    CSW_ADMIN_PASSWORD_HASH: passwordHash
  };

  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
  const localEnv = [
    "# Local deployment credentials. Never commit this file.",
    ...Array.from(localValues.entries()).map(([key, value]) => `${key}=${value}`),
    ""
  ].join("\n");
  fs.writeFileSync(envPath, localEnv, "utf8");
  console.log("Administrator password hash updated locally.");
  console.log("Run: tcb config update fn csw_admin_api");
  console.log("Choose Merge update when prompted.");
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
