const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const envId = String(process.argv[2] || "").trim();
if (!envId) {
  console.error("Usage: node scripts/csw_smoke_admin.js <env-id>");
  process.exit(1);
}

function localPassword() {
  const envPath = path.resolve(__dirname, "..", ".env.local");
  const raw = fs.readFileSync(envPath, "utf8");
  const line = raw.split(/\r?\n/).find((item) => item.startsWith("CSW_ADMIN_PASSWORD="));
  if (!line) throw new Error(`Administrator password is missing from ${envPath}`);
  return line.slice("CSW_ADMIN_PASSWORD=".length);
}

async function readJson(response, label) {
  const text = await response.text();
  let value;
  try {
    value = JSON.parse(text);
  } catch (_) {
    throw new Error(`${label} returned invalid JSON (${response.status}): ${text.slice(0, 200)}`);
  }
  if (!response.ok) throw new Error(`${label} failed (${response.status}): ${value.message || value.msg || "unknown error"}`);
  return value;
}

function unwrap(value) {
  let current = value;
  for (let index = 0; index < 3; index += 1) {
    if (typeof current.response_data === "string") {
      current = JSON.parse(current.response_data);
      continue;
    }
    if (typeof current.body === "string") {
      current = JSON.parse(current.body);
      continue;
    }
    if (current.data && typeof current.data === "object" && current.data.success !== undefined) {
      current = current.data;
      continue;
    }
    break;
  }
  return current;
}

async function main() {
  const base = `https://${envId}.api.tcloudbasegateway.com`;
  const deviceId = `csw_admin_smoke_${crypto.randomBytes(10).toString("hex")}`;
  const login = await readJson(await fetch(`${base}/auth/v1/signin/anonymously`, {
    method: "POST",
    headers: { "x-device-id": deviceId }
  }), "Anonymous login");
  const call = async (payload) => {
    const response = await fetch(`${base}/v1/functions/csw_admin_api`, {
      method: "POST",
      headers: {
        Authorization: `${login.token_type || "Bearer"} ${login.access_token}`,
        "Content-Type": "application/json"
      },
      body: JSON.stringify(payload)
    });
    return unwrap(await readJson(response, "Administrator API"));
  };

  const adminLogin = await call({ action: "login", password: localPassword() });
  if (!adminLogin.success || !adminLogin.adminToken) throw new Error(adminLogin.message || "Administrator login failed.");
  const inventory = await call({ action: "listInventory", adminToken: adminLogin.adminToken });
  if (!inventory.success) throw new Error(inventory.message || "Inventory listing failed.");
  const logout = await call({ action: "logout", adminToken: adminLogin.adminToken });
  if (!logout.success) throw new Error(logout.message || "Administrator logout failed.");

  console.log("Administrator login: OK");
  console.log(`Inventory read: OK (${Array.isArray(inventory.items) ? inventory.items.length : 0} items)`);
  console.log("Administrator logout: OK");
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
