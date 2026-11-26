const crypto = require("crypto");
const cloudbase = require("@cloudbase/node-sdk");

const COLLECTIONS = {
  inventory: "csw_inventory_profiles",
  orders: "csw_orders",
  allocations: "csw_allocations",
  paymentEvents: "csw_payment_events",
  audit: "csw_audit_events"
};

const app = cloudbase.init({ env: cloudbase.SYMBOL_CURRENT_ENV });
const db = app.database();

class PublicError extends Error {
  constructor(message, code = "BAD_REQUEST") {
    super(message);
    this.publicMessage = message;
    this.code = code;
  }
}

function nowIso() {
  return new Date().toISOString();
}

function documentData(result) {
  if (!result || !result.data) return null;
  return Array.isArray(result.data) ? result.data[0] || null : result.data;
}

function safeEqual(left, right) {
  const a = Buffer.from(String(left || ""));
  const b = Buffer.from(String(right || ""));
  return a.length === b.length && crypto.timingSafeEqual(a, b);
}

function parseRequest(event) {
  const rawBody = typeof event.body === "string" ? event.body : JSON.stringify(event.body || event);
  let body;
  try {
    body = typeof event.body === "string" ? JSON.parse(event.body) : (event.body || event);
  } catch (_) {
    throw new PublicError("Invalid webhook JSON.");
  }
  const headers = event.headers || {};
  const signature = String(
    headers["x-csw-signature"] || headers["X-CSW-Signature"] || ""
  ).replace(/^sha256=/i, "");
  return { rawBody, body, signature };
}

function verifySignature(rawBody, signature) {
  const secret = String(process.env.CSW_PAYMENT_WEBHOOK_SECRET || "");
  if (!secret) throw new PublicError("Payment webhook secret is not configured.", "WEBHOOK_NOT_CONFIGURED");
  const expected = crypto.createHmac("sha256", secret).update(rawBody).digest("hex");
  if (!safeEqual(expected, signature)) throw new PublicError("Invalid webhook signature.", "UNAUTHORIZED");
}

function normalizePayment(body, rawBody) {
  const status = String(body.status || body.paymentStatus || "").toLowerCase();
  if (!["paid", "success", "completed"].includes(status)) {
    throw new PublicError("Payment is not confirmed.", "PAYMENT_NOT_CONFIRMED");
  }
  const provider = String(body.provider || "catfk").slice(0, 32);
  const providerEventId = String(body.providerEventId || body.eventId || body.tradeNo || "").trim();
  const tradeNo = String(body.tradeNo || body.trade_no || "").trim();
  const orderId = String(body.orderId || "").trim();
  const amount = Number(body.amount ?? body.totalAmount);
  const currency = String(body.currency || "CNY").toUpperCase();
  if (!providerEventId || (!tradeNo && !orderId) || !Number.isFinite(amount)) {
    throw new PublicError("Webhook payment fields are incomplete.");
  }
  return {
    provider,
    providerEventId,
    tradeNo,
    orderId,
    amount,
    currency,
    paidAt: String(body.paidAt || nowIso()),
    payloadSha256: crypto.createHash("sha256").update(rawBody).digest("hex")
  };
}

async function findOrder(payment) {
  if (payment.orderId) {
    const result = await db.collection(COLLECTIONS.orders).doc(payment.orderId).get();
    return result.data && result.data[0] || null;
  }
  const result = await db.collection(COLLECTIONS.orders).where({ tradeNo: payment.tradeNo }).limit(1).get();
  return result.data && result.data[0] || null;
}

async function findCandidate(order) {
  if (order.reservedProfileId) {
    const reservedResult = await db.collection(COLLECTIONS.inventory).doc(order.reservedProfileId).get();
    const reserved = reservedResult.data && reservedResult.data[0];
    if (reserved && reserved.status === "reserved" && reserved.reservedByOrderId === order._id) return reserved;
    return null;
  }
  const result = await db.collection(COLLECTIONS.inventory).where({ status: "available" }).limit(100).get();
  return (result.data || []).find(item => (item.eligibleProducts || []).includes(order.productId)) || null;
}

async function fulfill(payment) {
  const previousEvent = await db.collection(COLLECTIONS.paymentEvents).where({
    provider: payment.provider,
    providerEventId: payment.providerEventId
  }).limit(1).get();
  if (previousEvent.data && previousEvent.data[0]) {
    return { success: true, repeated: true, allocationId: previousEvent.data[0].allocationId || null };
  }

  const order = await findOrder(payment);
  if (!order) throw new PublicError("Order was not found.", "ORDER_NOT_FOUND");
  if (order.paymentStatus === "paid" && order.allocationId) {
    return { success: true, repeated: true, allocationId: order.allocationId };
  }
  if (Math.abs(Number(order.productSnapshot.price) - payment.amount) > 0.001 ||
      String(order.productSnapshot.currency || "CNY").toUpperCase() !== payment.currency) {
    throw new PublicError("Paid amount does not match the order.", "PAYMENT_AMOUNT_MISMATCH");
  }

  const candidate = await findCandidate(order);
  if (!candidate) throw new PublicError("No matching profile is available.", "INVENTORY_EMPTY");

  const allocationId = crypto.randomUUID();
  const paymentEventId = crypto.randomUUID();
  const issuedAtMs = Date.now();
  const issuedAt = new Date(issuedAtMs).toISOString();
  const expiresAtMs = issuedAtMs + Number(order.productSnapshot.durationHours || 0) * 60 * 60 * 1000;
  const expiresAt = new Date(expiresAtMs).toISOString();

  await db.runTransaction(async transaction => {
    const latestOrderResult = await transaction.collection(COLLECTIONS.orders).doc(order._id).get();
    const latestInventoryResult = await transaction.collection(COLLECTIONS.inventory).doc(candidate._id).get();
    const latestOrder = documentData(latestOrderResult);
    const latestInventory = documentData(latestInventoryResult);
    if (!latestOrder || latestOrder.paymentStatus !== "pending") {
      throw new PublicError("Order is no longer pending.", "ORDER_FINALIZED");
    }
    const inventoryMatchesReservation = latestOrder.reservedProfileId
      ? latestInventory && latestInventory.status === "reserved" && latestInventory.reservedByOrderId === latestOrder._id
      : latestInventory && latestInventory.status === "available";
    if (!inventoryMatchesReservation) {
      throw new PublicError("Selected profile is no longer available.", "INVENTORY_CONFLICT");
    }

    const deliverySnapshot = {
      profileId: latestInventory._id,
      displayName: latestInventory.displayName,
      generation: latestInventory.generation,
      fileId: latestInventory.fileId,
      blobPath: latestInventory.blobPath,
      bundleSha256: latestInventory.bundleSha256 || "",
      bundleBytes: latestInventory.bundleBytes || null,
      profileStateAtDelivery: latestInventory.uploadSnapshot || {},
      inventoryStatusBefore: latestInventory.status,
      issuedAt,
      expiresAt,
      capturedAt: issuedAt
    };
    const paymentSnapshot = {
      ...(latestOrder.paymentSnapshot || {}),
      provider: payment.provider,
      tradeNo: payment.tradeNo || latestOrder.tradeNo,
      providerEventId: payment.providerEventId,
      amount: payment.amount,
      currency: payment.currency,
      paidAt: payment.paidAt,
      confirmedAt: issuedAt,
      providerPayloadHash: payment.payloadSha256
    };

    await transaction.collection(COLLECTIONS.allocations).add({
      _id: allocationId,
      orderId: latestOrder._id,
      buyerUid: latestOrder.buyerUid,
      installationId: latestOrder.installationId,
      productId: latestOrder.productId,
      profileId: latestInventory._id,
      generation: latestInventory.generation,
      status: "active",
      issuedAt,
      issuedAtMs,
      expiresAt,
      expiresAtMs,
      initialUsage: latestInventory.uploadSnapshot || {},
      currentUsage: latestInventory.uploadSnapshot || {},
      buyerSnapshot: latestOrder.buyerSnapshot,
      productSnapshot: latestOrder.productSnapshot,
      paymentSnapshot,
      deliverySnapshot,
      closedReason: "",
      createdAt: issuedAt,
      updatedAt: issuedAt
    });
    await transaction.collection(COLLECTIONS.inventory).doc(latestInventory._id).update({
      status: "occupied",
      occupiedByAllocationId: allocationId,
      occupiedByBuyerUid: latestOrder.buyerUid,
      occupiedAt: issuedAt,
      reservedByOrderId: null,
      reservedByBuyerUid: null,
      reservedAt: null,
      reservationExpiresAt: null,
      reservationExpiresAtMs: null,
      lastAllocationId: allocationId,
      deliveryCount: Number(latestInventory.deliveryCount || 0) + 1,
      updatedAt: issuedAt
    });
    await transaction.collection(COLLECTIONS.orders).doc(latestOrder._id).update({
      tradeNo: payment.tradeNo || latestOrder.tradeNo,
      paymentStatus: "paid",
      fulfillmentStatus: "allocated",
      paymentSnapshot,
      deliverySnapshot,
      allocationId,
      paidAt: payment.paidAt,
      fulfilledAt: issuedAt,
      updatedAt: issuedAt
    });
    await transaction.collection(COLLECTIONS.paymentEvents).add({
      _id: paymentEventId,
      provider: payment.provider,
      providerEventId: payment.providerEventId,
      tradeNo: payment.tradeNo,
      orderId: latestOrder._id,
      allocationId,
      amount: payment.amount,
      currency: payment.currency,
      payloadSha256: payment.payloadSha256,
      receivedAt: issuedAt
    });
  });

  await db.collection(COLLECTIONS.audit).add({
    actorType: "system",
    actorUid: "csw_payment_webhook",
    eventType: "payment_confirmed_and_profile_allocated",
    entityType: "order",
    entityId: order._id,
    metadata: {
      allocationId,
      profileId: candidate._id,
      buyerUid: order.buyerUid,
      productId: order.productId,
      tradeNo: payment.tradeNo,
      amount: payment.amount,
      currency: payment.currency,
      deliverySnapshotSha256: candidate.bundleSha256 || ""
    },
    createdAt: issuedAt
  });
  return { success: true, repeated: false, orderId: order._id, allocationId };
}

exports.main = async (event = {}) => {
  try {
    const request = parseRequest(event);
    verifySignature(request.rawBody, request.signature);
    return await fulfill(normalizePayment(request.body, request.rawBody));
  } catch (error) {
    console.error("csw_payment_webhook failed", error && error.stack || error);
    return {
      success: false,
      code: error && error.code || "INTERNAL_ERROR",
      message: error && error.publicMessage || "Payment confirmation failed."
    };
  }
};
