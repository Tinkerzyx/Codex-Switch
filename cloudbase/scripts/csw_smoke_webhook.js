const crypto = require("crypto");
const fs = require("fs");
const path = require("path");

const envId = String(process.argv[2] || "").trim();
const endpoint = String(process.argv[3] || "").trim() ||
  "https://ocr-h5-demo-d5g8f2qpsf8c21544-1441954144.ap-shanghai.app.tcloudbase.com/csw-payment-webhook";
if (!envId) {
  console.error("Usage: node scripts/csw_smoke_webhook.js <env-id>");
  process.exit(1);
}

function localSecret() {
  const envPath = path.resolve(__dirname, "..", ".env.local");
  const raw = fs.readFileSync(envPath, "utf8");
  const line = raw.split(/\r?\n/).find((item) => item.startsWith("CSW_PAYMENT_WEBHOOK_SECRET="));
  if (!line) throw new Error(`Payment webhook secret is missing from ${envPath}`);
  return line.slice("CSW_PAYMENT_WEBHOOK_SECRET=".length);
}

async function main() {
  const body = JSON.stringify({
    provider: "deployment-smoke",
    providerEventId: `smoke_${crypto.randomBytes(10).toString("hex")}`,
    orderId: `missing_${crypto.randomBytes(10).toString("hex")}`,
    status: "paid",
    amount: 62,
    currency: "CNY",
    paidAt: new Date().toISOString()
  });
  const signature = crypto.createHmac("sha256", localSecret()).update(body).digest("hex");
  const response = await fetch(
    endpoint,
    {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "x-csw-signature": signature
      },
      body
    }
  );
  const text = await response.text();
  const result = JSON.parse(text);
  if (!response.ok) throw new Error(`Webhook route failed (${response.status}): ${text.slice(0, 240)}`);
  if (result.code !== "ORDER_NOT_FOUND") {
    throw new Error(`Webhook signature test returned an unexpected result: ${text.slice(0, 240)}`);
  }
  console.log("Webhook route: OK");
  console.log("Webhook signature: OK");
  console.log("Expected missing-order response: OK");
}

main().catch((error) => {
  console.error(error.message || error);
  process.exit(1);
});
