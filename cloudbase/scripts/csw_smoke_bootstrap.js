const crypto = require("crypto");

const envId = String(process.argv[2] || "").trim();
if (!envId) {
  console.error("Usage: node scripts/csw_smoke_bootstrap.js <env-id>");
  process.exit(1);
}

async function readJson(response, label) {
  const text = await response.text();
  let value;
  try {
    value = JSON.parse(text);
  } catch (_) {
    throw new Error(`${label} returned invalid JSON (${response.status}): ${text.slice(0, 200)}`);
  }
  if (!response.ok) {
    throw new Error(`${label} failed (${response.status}): ${value.message || value.msg || "unknown error"}`);
  }
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
  const installationId = `csw_smoke_${crypto.randomBytes(12).toString("hex")}`;
  const loginResponse = await fetch(`${base}/auth/v1/signin/anonymously`, {
    method: "POST",
    headers: { "x-device-id": installationId }
  });
  const login = await readJson(loginResponse, "Anonymous login");
  if (!login.access_token) throw new Error("Anonymous login did not return an access token.");

  const invokeResponse = await fetch(`${base}/v1/functions/csw_client_api`, {
    method: "POST",
    headers: {
      Authorization: `${login.token_type || "Bearer"} ${login.access_token}`,
      "Content-Type": "application/json"
    },
    body: JSON.stringify({
      action: "bootstrap",
      installationId,
      appVersion: "deployment-smoke"
    })
  });
  const result = unwrap(await readJson(invokeResponse, "Client bootstrap"));
  if (!result || result.success !== true) {
    throw new Error(`Client bootstrap failed: ${result && result.message || "unknown error"}`);
  }
  const monthly = (result.products || []).find((product) => product.id === "monthly_30d");
  if (!monthly || Number(monthly.price) !== 62) throw new Error("Monthly cloud price is not 62 CNY.");
  if (typeof result.storePageHtml !== "string" ||
      !result.storePageHtml.includes("codex-switch-store-pay") ||
      !result.storePageHtml.includes("store-plan-card") ||
      !result.storePageHtml.includes("store-feature-list") ||
      !result.storePageHtml.includes("store-checkout-card")) {
    throw new Error("Cloud store page was not returned.");
  }
  console.log(`Anonymous login: OK`);
  console.log(`Client bootstrap: OK`);
  console.log(`Products seeded: ${Array.isArray(result.products) ? result.products.length : 0}`);
  console.log(`Monthly cloud price: ${monthly.price} ${monthly.currency}`);
  console.log(`Cloud store page: OK`);
  console.log(`Smoke installation: ${installationId}`);
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
