const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const writeMode = process.argv.includes("--write");
const suppliedPassword = process.argv.slice(2).find((value) => value !== "--write");
const password = String(suppliedPassword || crypto.randomBytes(18).toString("base64url"));
if (password.length < 10) {
  console.error("Usage: node scripts/csw_generate_secrets.js [admin-password-at-least-10-characters] [--write]");
  process.exit(1);
}

const salt = crypto.randomBytes(16);
const passwordHash = crypto.scryptSync(password, salt, 64);
const values = {
  CSW_ADMIN_PASSWORD_SALT: salt.toString("base64"),
  CSW_ADMIN_PASSWORD_HASH: passwordHash.toString("base64"),
  CSW_PROFILE_MASTER_KEY: crypto.randomBytes(32).toString("base64"),
  CSW_ACCOUNT_HASH_KEY: crypto.randomBytes(32).toString("base64"),
  CSW_PAYMENT_WEBHOOK_SECRET: crypto.randomBytes(32).toString("base64url")
};

if (writeMode) {
  const root = path.resolve(__dirname, "..");
  const configPath = path.join(root, "cloudbaserc.json");
  if (!fs.existsSync(configPath)) {
    console.error(`Missing deployment config: ${configPath}`);
    process.exit(1);
  }
  const config = JSON.parse(fs.readFileSync(configPath, "utf8"));
  for (const fn of config.functions || []) {
    if (fn.name === "csw_admin_api") {
      fn.envVariables = {
        CSW_ADMIN_PASSWORD_SALT: values.CSW_ADMIN_PASSWORD_SALT,
        CSW_ADMIN_PASSWORD_HASH: values.CSW_ADMIN_PASSWORD_HASH,
        CSW_PROFILE_MASTER_KEY: values.CSW_PROFILE_MASTER_KEY,
        CSW_ACCOUNT_HASH_KEY: values.CSW_ACCOUNT_HASH_KEY
      };
    }
    if (fn.name === "csw_payment_webhook") {
      fn.envVariables = {
        CSW_PAYMENT_WEBHOOK_SECRET: values.CSW_PAYMENT_WEBHOOK_SECRET
      };
    }
  }
  fs.writeFileSync(configPath, `${JSON.stringify(config, null, 2)}\n`, "utf8");
  const localEnv = [
    "# Local deployment credentials. Never commit this file.",
    `CSW_ADMIN_PASSWORD=${password}`,
    ...Object.entries(values).map(([key, value]) => `${key}=${value}`),
    ""
  ].join("\n");
  const envPath = path.join(root, ".env.local");
  fs.writeFileSync(envPath, localEnv, "utf8");
  console.log(`Deployment secrets were written to ${configPath}`);
  console.log(`The initial administrator password is stored in ${envPath}`);
  process.exit(0);
}

console.log("Configure these values as CloudBase function environment variables:");
for (const [key, value] of Object.entries(values)) console.log(`${key}=${value}`);
