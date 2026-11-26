const crypto = require("crypto");
const fs = require("fs");
const path = require("path");
const cloudbase = require("@cloudbase/node-sdk");

const COLLECTIONS = {
  devices: "csw_devices",
  products: "csw_products",
  inventory: "csw_inventory_profiles",
  orders: "csw_orders",
  allocations: "csw_allocations",
  paymentEvents: "csw_payment_events",
  adminSessions: "csw_admin_sessions",
  audit: "csw_audit_events",
  rateLimits: "csw_rate_limits"
};

const app = cloudbase.init({ env: cloudbase.SYMBOL_CURRENT_ENV });
const db = app.database();
const RESERVATION_MS = 20 * 60 * 1000;
const CATFK_POLL_URL = "https://catfk.com/shopApi/Pay/query";
const STORE_PAGE_TEMPLATE = fs.readFileSync(path.join(__dirname, "store-page.html"), "utf8");
const STORE_PAGE_CSS = fs.readFileSync(path.join(__dirname, "store-page.css"), "utf8");

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

function masterKey() {
  const key = Buffer.from(String(process.env.CSW_PROFILE_MASTER_KEY || ""), "base64");
  if (key.length !== 32) throw new PublicError("Profile delivery is not configured.", "DELIVERY_NOT_CONFIGURED");
  return key;
}

function decryptBundle(fileContent, expectedSha256) {
  const encrypted = Buffer.isBuffer(fileContent) ? fileContent : Buffer.from(fileContent || "");
  const actualSha256 = crypto.createHash("sha256").update(encrypted).digest("hex");
  if (expectedSha256 && actualSha256 !== String(expectedSha256).toLowerCase()) {
    throw new PublicError("Profile bundle integrity verification failed.", "DELIVERY_INTEGRITY_FAILED");
  }
  let envelope;
  try {
    envelope = JSON.parse(encrypted.toString("utf8"));
  } catch (_) {
    throw new PublicError("Profile bundle is invalid.", "DELIVERY_INVALID");
  }
  if (Number(envelope.version) !== 1 || envelope.algorithm !== "AES-256-GCM") {
    throw new PublicError("Profile bundle format is not supported.", "DELIVERY_INVALID");
  }
  try {
    const decipher = crypto.createDecipheriv("aes-256-gcm", masterKey(), Buffer.from(envelope.iv, "base64"));
    decipher.setAuthTag(Buffer.from(envelope.tag, "base64"));
    const plaintext = Buffer.concat([
      decipher.update(Buffer.from(envelope.ciphertext, "base64")),
      decipher.final()
    ]);
    return JSON.parse(plaintext.toString("utf8"));
  } catch (_) {
    throw new PublicError("Profile bundle could not be decrypted.", "DELIVERY_DECRYPT_FAILED");
  }
}

async function ensureCollection(name) {
  try {
    await db.createCollection(name);
  } catch (error) {
    const message = String(error && error.message || error || "");
    if (!/exist|already|duplicate/i.test(message)) throw error;
  }
}

async function ensureCollections() {
  for (const name of Object.values(COLLECTIONS)) await ensureCollection(name);
}

const PRODUCT_SEEDS = [
  {
    _id: "monthly_30d",
    name: "Codex 月度通行证",
    price: 62,
    pricingVersion: 2,
    currency: "CNY",
    descriptionZh: "30 天 Codex 额度，使用官方完整订阅账号。",
    descriptionEn: "30 days of Codex access on an official subscription account.",
    feature1Zh: "30天使用权限",
    feature1En: "30-day access",
    feature2Zh: "官方全价订阅Plus账号",
    feature2En: "Official full-price Plus account",
    durationHours: 720,
    depletionPolicy: "ignore",
    goodsKey: "bpx5pe",
    itemUrl: "https://catfk.com/item/bpx5pe",
    enabled: true,
    schemaVersion: 1
  },
  {
    _id: "weekly_100_3d",
    name: "Codex 单次周额度",
    price: 20,
    currency: "CNY",
    descriptionZh: "单次周额度，适合短期使用。",
    descriptionEn: "A single weekly quota for short-term use.",
    feature1Zh: "单次100%额度，有效期7天",
    feature1En: "One-time 100% quota, valid for 7 days",
    feature2Zh: "官方全价订阅Plus账号",
    feature2En: "Official full-price Plus account",
    durationHours: 168,
    depletionPolicy: "remove_when_zero",
    goodsKey: "t133lf",
    itemUrl: "https://catfk.com/item/t133lf",
    enabled: true,
    schemaVersion: 1
  }
];

async function seedProducts() {
  for (const product of PRODUCT_SEEDS) {
    const existing = await db.collection(COLLECTIONS.products).doc(product._id).get();
    if (existing.data && existing.data.length) {
      const current = existing.data[0];
      const missingContent = {};
      for (const field of ["descriptionZh", "descriptionEn", "feature1Zh", "feature1En", "feature2Zh", "feature2En"]) {
        if (current[field] === undefined) missingContent[field] = product[field];
      }
      if (product._id === "monthly_30d" && Number(current.pricingVersion || 0) < 2) {
        missingContent.price = 62;
        missingContent.pricingVersion = 2;
      }
      if (Object.keys(missingContent).length) {
        await db.collection(COLLECTIONS.products).doc(product._id).update({ ...missingContent, updatedAt: nowIso() });
      }
      continue;
    }
    await db.collection(COLLECTIONS.products).add({ ...product, createdAt: nowIso(), updatedAt: nowIso() });
  }
}

function authUser() {
  const userInfo = app.auth().getUserInfo();
  const uid = String(userInfo && userInfo.uid || "").trim();
  if (!uid) throw new PublicError("CloudBase anonymous identity is required.", "UNAUTHORIZED");
  return { uid };
}

function cleanInstallationId(value) {
  const id = String(value || "").trim();
  if (!/^[a-zA-Z0-9_-]{12,96}$/.test(id)) {
    throw new PublicError("Invalid installation identity.");
  }
  return id;
}

async function upsertDevice(uid, data) {
  const installationId = cleanInstallationId(data.installationId);
  const existing = await db.collection(COLLECTIONS.devices).where({ uid, installationId }).limit(1).get();
  const now = nowIso();
  const document = {
    uid,
    installationId,
    appVersion: String(data.appVersion || "").slice(0, 32),
    platform: "windows",
    lastSeenAt: now,
    revoked: false
  };
  if (existing.data && existing.data[0]) {
    await db.collection(COLLECTIONS.devices).doc(existing.data[0]._id).update(document);
    return { ...existing.data[0], ...document };
  }
  const deviceId = crypto.randomUUID();
  await db.collection(COLLECTIONS.devices).add({ _id: deviceId, ...document, createdAt: now });
  return { _id: deviceId, ...document, createdAt: now };
}

function publicProduct(product, availableCount = 0) {
  return {
    id: product._id,
    name: product.name,
    price: product.price,
    descriptionZh: product.descriptionZh || "",
    descriptionEn: product.descriptionEn || "",
    feature1Zh: product.feature1Zh || "",
    feature1En: product.feature1En || "",
    feature2Zh: product.feature2Zh || "",
    feature2En: product.feature2En || "",
    goodsKey: product.goodsKey,
    itemUrl: product.itemUrl,
    currency: product.currency,
    durationHours: product.durationHours,
    depletionPolicy: product.depletionPolicy,
    enabled: product.enabled !== false,
    availableCount: Number(availableCount || 0),
    available: Number(availableCount || 0) > 0
  };
}

function renderStorePage(products) {
  const pageData = {
    products: products.map((product) => ({
      id: product.id,
      name: product.name,
      price: Number(product.price),
      currency: product.currency,
      descriptionZh: product.descriptionZh,
      descriptionEn: product.descriptionEn,
      feature1Zh: product.feature1Zh,
      feature1En: product.feature1En,
      feature2Zh: product.feature2Zh,
      feature2En: product.feature2En,
      availableCount: Number(product.availableCount || 0)
    }))
  };
  return STORE_PAGE_TEMPLATE
    .replace("__STORE_CSS__", STORE_PAGE_CSS)
    .replace("__STORE_DATA__", JSON.stringify(pageData).replace(/</g, "\\u003c"));
}

function inventoryCounts(items) {
  const counts = { monthly_30d: 0, weekly_100_3d: 0 };
  for (const item of items || []) {
    for (const productId of item.eligibleProducts || []) {
      if (Object.prototype.hasOwnProperty.call(counts, productId)) counts[productId] += 1;
    }
  }
  return counts;
}

async function bootstrap(data) {
  await ensureCollections();
  await seedProducts();
  const user = authUser();
  const device = await upsertDevice(user.uid, data);
  const [productsResult, inventoryResult] = await Promise.all([
    db.collection(COLLECTIONS.products).where({ enabled: true }).get(),
    db.collection(COLLECTIONS.inventory).where({ status: "available" }).limit(100).get()
  ]);
  const counts = inventoryCounts(inventoryResult.data || []);
  return {
    success: true,
    identity: {
      uid: user.uid,
      deviceId: device._id,
      installationId: device.installationId
    },
    products: (productsResult.data || []).map((product) => publicProduct(product, counts[product._id] || 0)),
    storePageHtml: renderStorePage((productsResult.data || []).map((product) => publicProduct(product, counts[product._id] || 0))),
    schemaVersion: 1
  };
}

async function syncEntitlements(data) {
  const user = authUser();
  const installationId = cleanInstallationId(data.installationId);
  const result = await db.collection(COLLECTIONS.allocations).where({
    buyerUid: user.uid,
    installationId
  }).limit(100).get();
  return {
    success: true,
    allocations: (result.data || []).map((item) => ({
      id: item._id,
      orderId: item.orderId,
      profileId: item.profileId,
      productId: item.productId,
      status: item.status,
      generation: item.generation,
      issuedAt: item.issuedAt,
      expiresAt: item.expiresAt,
      initialUsage: item.initialUsage,
      currentUsage: item.currentUsage,
      closedReason: item.closedReason || ""
    }))
  };
}

async function releaseOrderReservation(order, reason) {
  if (!order || order.paymentStatus !== "pending" || !order.reservedProfileId) return false;
  let released = false;
  const now = nowIso();
  await db.runTransaction(async transaction => {
    const orderRef = transaction.collection(COLLECTIONS.orders).doc(order._id);
    const latestOrder = documentData(await orderRef.get());
    if (!latestOrder || latestOrder.paymentStatus !== "pending" || !latestOrder.reservedProfileId) return;
    const inventoryRef = transaction.collection(COLLECTIONS.inventory).doc(latestOrder.reservedProfileId);
    const latestInventory = documentData(await inventoryRef.get());
    if (latestInventory && latestInventory.status === "reserved" && latestInventory.reservedByOrderId === latestOrder._id) {
      await inventoryRef.update({
        status: "available",
        reservedByOrderId: null,
        reservedByBuyerUid: null,
        reservedAt: null,
        reservationExpiresAt: null,
        reservationExpiresAtMs: null,
        updatedAt: now
      });
    }
    await orderRef.update({
      fulfillmentStatus: "released",
      reservedProfileId: null,
      reservationExpiresAt: null,
      reservationExpiresAtMs: null,
      abandonedAt: now,
      abandonedReason: reason,
      updatedAt: now
    });
    released = true;
  });
  return released;
}

async function abandonPreviousOrders(user, installationId, reason) {
  const result = await db.collection(COLLECTIONS.orders).where({ buyerUid: user.uid }).limit(100).get();
  const pending = (result.data || []).filter(item =>
    item.installationId === installationId &&
    item.paymentStatus === "pending" &&
    item.reservedProfileId
  );
  for (const order of pending) await releaseOrderReservation(order, reason);
}

async function queryCatfkPayment(order) {
  const tradeNo = String(order.tradeNo || "").trim();
  if (!tradeNo) return { paid: false, pending: true, raw: null };
  const response = await fetch(CATFK_POLL_URL, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      origin: "https://catfk.com",
      referer: String(order.productSnapshot && order.productSnapshot.itemUrl || "https://catfk.com/")
    },
    body: JSON.stringify({ trade_no: tradeNo })
  });
  if (!response.ok) throw new PublicError(`Payment verification returned HTTP ${response.status}.`, "PAYMENT_VERIFY_FAILED");
  const result = await response.json();
  const message = String(result && result.msg || "").toLowerCase();
  return {
    paid: Number(result && result.code) === 1 && message === "success",
    pending: Number(result && result.code) === 0 && message === "not pay",
    raw: result
  };
}

async function allocateVerifiedOrder(order, providerResult) {
  const inventoryResult = await db.collection(COLLECTIONS.inventory).where({ status: "available" }).limit(100).get();
  let candidate = null;
  if (order.reservedProfileId) {
    const reservedResult = await db.collection(COLLECTIONS.inventory).doc(order.reservedProfileId).get();
    const reserved = documentData(reservedResult);
    if (reserved && reserved.status === "reserved" && reserved.reservedByOrderId === order._id) candidate = reserved;
  } else {
    candidate = (inventoryResult.data || []).find(item => (item.eligibleProducts || []).includes(order.productId)) || null;
  }
  if (!candidate) throw new PublicError("Payment succeeded, but no matching profile is currently available.", "PAID_INVENTORY_EMPTY");

  const allocationId = crypto.randomUUID();
  const paymentEventId = crypto.randomUUID();
  const issuedAtMs = Date.now();
  const issuedAt = new Date(issuedAtMs).toISOString();
  const expiresAtMs = issuedAtMs + Number(order.productSnapshot.durationHours || 0) * 60 * 60 * 1000;
  const expiresAt = new Date(expiresAtMs).toISOString();
  let completedAllocationId = allocationId;
  await db.runTransaction(async transaction => {
    const orderRef = transaction.collection(COLLECTIONS.orders).doc(order._id);
    const inventoryRef = transaction.collection(COLLECTIONS.inventory).doc(candidate._id);
    const latestOrder = documentData(await orderRef.get());
    const latestInventory = documentData(await inventoryRef.get());
    if (latestOrder && latestOrder.paymentStatus === "paid" && latestOrder.allocationId) {
      completedAllocationId = latestOrder.allocationId;
      return;
    }
    if (!latestOrder || latestOrder.paymentStatus !== "pending") {
      throw new PublicError("Order can no longer be fulfilled.", "ORDER_FINALIZED");
    }
    const inventoryMatches = latestOrder.reservedProfileId
      ? latestInventory && latestInventory.status === "reserved" && latestInventory.reservedByOrderId === latestOrder._id
      : latestInventory && latestInventory.status === "available" && (latestInventory.eligibleProducts || []).includes(latestOrder.productId);
    if (!inventoryMatches) throw new PublicError("Selected profile is no longer available.", "INVENTORY_CONFLICT");

    const paymentSnapshot = {
      ...(latestOrder.paymentSnapshot || {}),
      provider: "catfk",
      tradeNo: latestOrder.tradeNo,
      providerEventId: `catfk-query:${latestOrder.tradeNo}`,
      amount: latestOrder.productSnapshot.price,
      currency: latestOrder.productSnapshot.currency || "CNY",
      paidAt: issuedAt,
      confirmedAt: issuedAt,
      confirmationSource: "provider_query"
    };
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
    await inventoryRef.update({
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
    await orderRef.update({
      paymentStatus: "paid",
      fulfillmentStatus: "allocated",
      paymentSnapshot,
      deliverySnapshot,
      allocationId,
      paidAt: issuedAt,
      fulfilledAt: issuedAt,
      updatedAt: issuedAt
    });
    await transaction.collection(COLLECTIONS.paymentEvents).add({
      _id: paymentEventId,
      provider: "catfk",
      providerEventId: `catfk-query:${latestOrder.tradeNo}`,
      tradeNo: latestOrder.tradeNo,
      orderId: latestOrder._id,
      allocationId,
      amount: latestOrder.productSnapshot.price,
      currency: latestOrder.productSnapshot.currency || "CNY",
      payloadSha256: crypto.createHash("sha256").update(JSON.stringify(providerResult || {})).digest("hex"),
      receivedAt: issuedAt
    });
  });
  return completedAllocationId;
}

async function reconcilePayment(data) {
  const user = authUser();
  const installationId = cleanInstallationId(data.installationId);
  const orderId = String(data.orderId || "").trim();
  const orderResult = await db.collection(COLLECTIONS.orders).doc(orderId).get();
  const order = documentData(orderResult);
  if (!order || order.buyerUid !== user.uid || order.installationId !== installationId) {
    throw new PublicError("Order was not found.", "NOT_FOUND");
  }
  if (order.paymentStatus === "paid" && order.allocationId) {
    return { success: true, paid: true, allocationId: order.allocationId, order };
  }
  const provider = await queryCatfkPayment(order);
  if (!provider.paid) return { success: true, paid: false, pending: provider.pending, order };
  const allocationId = await allocateVerifiedOrder(order, provider.raw);
  const refreshed = documentData(await db.collection(COLLECTIONS.orders).doc(orderId).get());
  await db.collection(COLLECTIONS.audit).add({
    actorType: "system",
    actorUid: "csw_client_api",
    eventType: "payment_reconciled_and_profile_allocated",
    entityType: "order",
    entityId: orderId,
    metadata: { allocationId, tradeNo: order.tradeNo, productId: order.productId },
    createdAt: nowIso()
  });
  return { success: true, paid: true, allocationId, order: refreshed };
}

async function abandonOrder(data) {
  const user = authUser();
  const installationId = cleanInstallationId(data.installationId);
  const orderId = String(data.orderId || "").trim();
  const result = await db.collection(COLLECTIONS.orders).doc(orderId).get();
  const order = documentData(result);
  if (!order || order.buyerUid !== user.uid || order.installationId !== installationId) {
    throw new PublicError("Order was not found.", "NOT_FOUND");
  }
  const released = await releaseOrderReservation(order, "client_cancelled");
  return { success: true, released, orderId };
}

async function claimDelivery(data) {
  const user = authUser();
  const installationId = cleanInstallationId(data.installationId);
  const allocationId = String(data.allocationId || "").trim();
  if (!allocationId) throw new PublicError("Allocation ID is required.");

  const allocationResult = await db.collection(COLLECTIONS.allocations).doc(allocationId).get();
  const allocation = allocationResult.data && allocationResult.data[0];
  if (!allocation || allocation.buyerUid !== user.uid || allocation.installationId !== installationId) {
    throw new PublicError("Delivery was not found.", "NOT_FOUND");
  }
  if (allocation.status !== "active" || Number(allocation.expiresAtMs || 0) <= Date.now()) {
    throw new PublicError("This delivery authorization has expired.", "DELIVERY_EXPIRED");
  }

  const inventoryResult = await db.collection(COLLECTIONS.inventory).doc(allocation.profileId).get();
  const inventory = inventoryResult.data && inventoryResult.data[0];
  if (!inventory || inventory.status !== "occupied" || inventory.occupiedByAllocationId !== allocationId) {
    throw new PublicError("Allocated profile is not available for delivery.", "DELIVERY_UNAVAILABLE");
  }
  if (Number(inventory.generation || 0) !== Number(allocation.generation || 0) || !inventory.fileId) {
    throw new PublicError("Allocated profile generation does not match.", "DELIVERY_GENERATION_MISMATCH");
  }

  const download = await app.downloadFile({ fileID: inventory.fileId });
  const bundle = decryptBundle(download.fileContent, inventory.bundleSha256);
  if (String(bundle.profileId || "") !== String(inventory._id) || Number(bundle.generation || 0) !== Number(inventory.generation || 0)) {
    throw new PublicError("Delivered profile identity does not match the allocation.", "DELIVERY_INTEGRITY_FAILED");
  }
  const authText = String(bundle.authText || "");
  const configText = String(bundle.configText || "");
  if (Buffer.byteLength(authText, "utf8") <= 3 * 1024 || Buffer.byteLength(configText, "utf8") <= 2 * 1024) {
    throw new PublicError("Delivered profile files are incomplete.", "DELIVERY_INVALID");
  }

  const now = nowIso();
  const claimCount = Number(allocation.deliveryClaimCount || 0) + 1;
  await db.collection(COLLECTIONS.allocations).doc(allocationId).update({
    deliveredAt: allocation.deliveredAt || now,
    lastDeliveryClaimAt: now,
    deliveryClaimCount: claimCount,
    updatedAt: now
  });
  await db.collection(COLLECTIONS.audit).add({
    actorType: "user",
    actorUid: user.uid,
    eventType: "profile_delivery_claimed",
    entityType: "allocation",
    entityId: allocationId,
    metadata: {
      orderId: allocation.orderId,
      profileId: allocation.profileId,
      productId: allocation.productId,
      installationId,
      generation: allocation.generation,
      claimCount
    },
    createdAt: now
  });

  return {
    success: true,
    delivery: {
      allocationId,
      orderId: allocation.orderId,
      productId: allocation.productId,
      profileId: allocation.profileId,
      generation: allocation.generation,
      profileName: String(bundle.profileName || inventory.displayName || "Codex Profile"),
      authText,
      configText,
      authSha256: crypto.createHash("sha256").update(authText, "utf8").digest("hex"),
      configSha256: crypto.createHash("sha256").update(configText, "utf8").digest("hex"),
      issuedAt: allocation.issuedAt,
      expiresAt: allocation.expiresAt,
      productSnapshot: allocation.productSnapshot || {},
      initialUsage: allocation.initialUsage || {}
    }
  };
}

async function createOrderDraft(data) {
  const user = authUser();
  const installationId = cleanInstallationId(data.installationId);
  const productId = String(data.productId || "").trim();
  const clientOrderId = String(data.clientOrderId || "").trim();
  if (!/^[a-zA-Z0-9_-]{12,96}$/.test(clientOrderId)) throw new PublicError("Invalid client order ID.");

  const repeated = await db.collection(COLLECTIONS.orders).where({ clientOrderId, buyerUid: user.uid }).limit(1).get();
  if (repeated.data && repeated.data[0]) return { success: true, order: repeated.data[0], repeated: true };

  await abandonPreviousOrders(user, installationId, "superseded_by_new_order");

  const productResult = await db.collection(COLLECTIONS.products).doc(productId).get();
  const product = productResult.data && productResult.data[0];
  if (!product || product.enabled === false) throw new PublicError("Product is not available.", "PRODUCT_UNAVAILABLE");

  const now = nowIso();
  const reservationExpiresAtMs = Date.now() + RESERVATION_MS;
  const reservationExpiresAt = new Date(reservationExpiresAtMs).toISOString();
  const order = {
    _id: crypto.randomUUID(),
    clientOrderId,
    buyerUid: user.uid,
    installationId,
    productId,
    paymentStatus: "pending",
    fulfillmentStatus: "reserved",
    buyerSnapshot: {
      uid: user.uid,
      installationId,
      appVersion: String(data.appVersion || "").slice(0, 32),
      capturedAt: now
    },
    productSnapshot: {
      id: product._id,
      name: product.name,
      price: product.price,
      currency: product.currency,
      durationHours: product.durationHours,
      depletionPolicy: product.depletionPolicy,
      goodsKey: product.goodsKey,
      itemUrl: product.itemUrl,
      capturedAt: now
    },
    paymentSnapshot: {
      provider: "catfk",
      tradeNo: "",
      checkoutCreatedAt: null,
      amount: product.price,
      currency: product.currency,
      paidAt: null,
      providerPayloadHash: ""
    },
    deliverySnapshot: null,
    allocationId: null,
    reservedProfileId: null,
    reservationExpiresAt,
    reservationExpiresAtMs,
    createdAt: now,
    updatedAt: now
  };
  const inventoryResult = await db.collection(COLLECTIONS.inventory).where({ status: "available" }).limit(100).get();
  const candidates = (inventoryResult.data || []).filter((item) => (item.eligibleProducts || []).includes(productId));
  for (const candidate of candidates) {
    try {
      const reservedOrder = { ...order, reservedProfileId: candidate._id };
      await db.runTransaction(async transaction => {
        const latestResult = await transaction.collection(COLLECTIONS.inventory).doc(candidate._id).get();
        const latest = documentData(latestResult);
        if (!latest || latest.status !== "available" || !(latest.eligibleProducts || []).includes(productId)) {
          throw new PublicError("Inventory candidate is no longer available.", "INVENTORY_CONFLICT");
        }
        await transaction.collection(COLLECTIONS.inventory).doc(candidate._id).update({
          status: "reserved",
          reservedByOrderId: order._id,
          reservedByBuyerUid: user.uid,
          reservedAt: now,
          reservationExpiresAt,
          reservationExpiresAtMs,
          updatedAt: now
        });
        await transaction.collection(COLLECTIONS.orders).add(reservedOrder);
      });
      return { success: true, order: reservedOrder };
    } catch (error) {
      if (!(error instanceof PublicError) || error.code !== "INVENTORY_CONFLICT") throw error;
    }
  }
  throw new PublicError("No matching profile is available.", "INVENTORY_EMPTY");
}

async function attachPaymentReference(data) {
  const user = authUser();
  const orderId = String(data.orderId || "").trim();
  const tradeNo = String(data.tradeNo || "").trim();
  if (!orderId || !/^[a-zA-Z0-9_-]{6,128}$/.test(tradeNo)) {
    throw new PublicError("Invalid payment reference.");
  }
  const orderResult = await db.collection(COLLECTIONS.orders).doc(orderId).get();
  const order = orderResult.data && orderResult.data[0];
  if (!order || order.buyerUid !== user.uid) throw new PublicError("Order was not found.", "NOT_FOUND");
  if (order.paymentStatus !== "pending") throw new PublicError("Order can no longer be changed.", "ORDER_FINALIZED");

  const duplicate = await db.collection(COLLECTIONS.orders).where({ tradeNo }).limit(1).get();
  if (duplicate.data && duplicate.data[0] && duplicate.data[0]._id !== orderId) {
    throw new PublicError("Payment reference is already attached to another order.", "PAYMENT_REFERENCE_DUPLICATE");
  }
  const now = nowIso();
  const paymentSnapshot = {
    ...(order.paymentSnapshot || {}),
    provider: "catfk",
    tradeNo,
    checkoutCreatedAt: now
  };
  await db.collection(COLLECTIONS.orders).doc(orderId).update({
    tradeNo,
    paymentSnapshot,
    updatedAt: now
  });
  return { success: true, orderId, tradeNo };
}

async function getOrder(data) {
  const user = authUser();
  const orderId = String(data.orderId || "").trim();
  const result = await db.collection(COLLECTIONS.orders).doc(orderId).get();
  const order = result.data && result.data[0];
  if (!order || order.buyerUid !== user.uid) throw new PublicError("Order was not found.", "NOT_FOUND");
  return { success: true, order };
}

exports.main = async (event = {}) => {
  try {
    const action = String(event.action || "bootstrap");
    if (action === "bootstrap") return await bootstrap(event);
    if (action === "syncEntitlements") return await syncEntitlements(event);
    if (action === "claimDelivery") return await claimDelivery(event);
    if (action === "reconcilePayment") return await reconcilePayment(event);
    if (action === "abandonOrder") return await abandonOrder(event);
    if (action === "createOrderDraft") return await createOrderDraft(event);
    if (action === "attachPaymentReference") return await attachPaymentReference(event);
    if (action === "getOrder") return await getOrder(event);
    throw new PublicError("Unknown client action.");
  } catch (error) {
    console.error("csw_client_api failed", error && error.stack || error);
    return {
      success: false,
      code: error && error.code || "INTERNAL_ERROR",
      message: error && error.publicMessage || "Cloud service request failed."
    };
  }
};
