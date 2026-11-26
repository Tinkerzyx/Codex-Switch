const crypto = require("crypto");
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
const ADMIN_SESSION_MS = 30 * 60 * 1000;
const LOCKOUT_MS = 15 * 60 * 1000;
const MAX_LOGIN_FAILURES = 5;
const MIN_AUTH_BYTES = 3 * 1024;
const MIN_CONFIG_BYTES = 2 * 1024;

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

function tokenHash(token) {
  return crypto.createHash("sha256").update(String(token || "")).digest("hex");
}

function safeEqual(left, right) {
  const a = Buffer.from(String(left || ""));
  const b = Buffer.from(String(right || ""));
  return a.length === b.length && crypto.timingSafeEqual(a, b);
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

function authUser() {
  const userInfo = app.auth().getUserInfo();
  const uid = String(userInfo && userInfo.uid || "").trim();
  if (!uid) throw new PublicError("CloudBase identity is required.", "UNAUTHORIZED");
  return { uid };
}

function passwordConfig() {
  const salt = String(process.env.CSW_ADMIN_PASSWORD_SALT || "").trim();
  const hash = String(process.env.CSW_ADMIN_PASSWORD_HASH || "").trim();
  if (!salt || !hash) throw new PublicError("Administrator password is not configured.", "ADMIN_NOT_CONFIGURED");
  return { salt, hash };
}

function verifyPassword(password) {
  const config = passwordConfig();
  const salt = Buffer.from(config.salt, "base64");
  const actual = crypto.scryptSync(String(password || ""), salt, 64).toString("base64");
  return safeEqual(actual, config.hash);
}

async function readRateLimit(uid) {
  const result = await db.collection(COLLECTIONS.rateLimits).where({ key: `admin-login:${uid}` }).limit(1).get();
  return result.data && result.data[0] || null;
}

async function writeRateLimit(uid, document) {
  const key = `admin-login:${uid}`;
  const existing = await readRateLimit(uid);
  if (existing) {
    await db.collection(COLLECTIONS.rateLimits).doc(existing._id).update({ ...document, key });
  } else {
    await db.collection(COLLECTIONS.rateLimits).add({ key, ...document });
  }
}

async function audit(actorUid, eventType, entityType, entityId, metadata = {}) {
  await db.collection(COLLECTIONS.audit).add({
    actorUid,
    actorType: "admin",
    eventType,
    entityType,
    entityId: String(entityId || ""),
    metadata,
    createdAt: nowIso()
  });
}

async function login(data) {
  await ensureCollections();
  const user = authUser();
  const rate = await readRateLimit(user.uid);
  const now = Date.now();
  if (rate && Number(rate.lockedUntil || 0) > now) {
    throw new PublicError("Administrator login is temporarily locked.", "ADMIN_LOCKED");
  }

  if (!verifyPassword(data.password)) {
    const failures = Number(rate && rate.failures || 0) + 1;
    const lockedUntil = failures >= MAX_LOGIN_FAILURES ? now + LOCKOUT_MS : 0;
    await writeRateLimit(user.uid, { failures, lockedUntil, updatedAt: nowIso() });
    await audit(user.uid, "admin_login_failed", "admin_session", "", { failures, lockedUntil });
    throw new PublicError("Administrator password is incorrect.", "ADMIN_PASSWORD_INVALID");
  }

  await writeRateLimit(user.uid, { failures: 0, lockedUntil: 0, updatedAt: nowIso() });
  const token = crypto.randomBytes(32).toString("base64url");
  const sessionId = crypto.randomUUID();
  const expiresAtMs = now + ADMIN_SESSION_MS;
  await db.collection(COLLECTIONS.adminSessions).add({
    _id: sessionId,
    uid: user.uid,
    tokenHash: tokenHash(token),
    revoked: false,
    createdAt: nowIso(),
    expiresAt: new Date(expiresAtMs).toISOString(),
    expiresAtMs
  });
  await audit(user.uid, "admin_login_succeeded", "admin_session", sessionId);
  return { success: true, adminToken: token, expiresAt: new Date(expiresAtMs).toISOString(), uid: user.uid };
}

async function assertAdmin(data) {
  const user = authUser();
  const hash = tokenHash(data.adminToken);
  const result = await db.collection(COLLECTIONS.adminSessions).where({
    uid: user.uid,
    tokenHash: hash,
    revoked: false
  }).limit(1).get();
  const session = result.data && result.data[0];
  if (!session || Number(session.expiresAtMs || 0) <= Date.now()) {
    throw new PublicError("Administrator session has expired.", "ADMIN_SESSION_EXPIRED");
  }
  return { uid: user.uid, session };
}

function masterKey() {
  const key = Buffer.from(String(process.env.CSW_PROFILE_MASTER_KEY || ""), "base64");
  if (key.length !== 32) throw new PublicError("Profile encryption key is not configured.", "ENCRYPTION_NOT_CONFIGURED");
  return key;
}

function accountHash(identity) {
  const hashKey = String(process.env.CSW_ACCOUNT_HASH_KEY || process.env.CSW_PROFILE_MASTER_KEY || "");
  if (!hashKey) throw new PublicError("Account hash key is not configured.", "ENCRYPTION_NOT_CONFIGURED");
  return crypto.createHmac("sha256", hashKey).update(String(identity || "").trim().toLowerCase()).digest("hex");
}

function encryptBundle(bundle) {
  const iv = crypto.randomBytes(12);
  const cipher = crypto.createCipheriv("aes-256-gcm", masterKey(), iv);
  const encrypted = Buffer.concat([cipher.update(JSON.stringify(bundle), "utf8"), cipher.final()]);
  return Buffer.from(JSON.stringify({
    version: 1,
    algorithm: "AES-256-GCM",
    iv: iv.toString("base64"),
    tag: cipher.getAuthTag().toString("base64"),
    ciphertext: encrypted.toString("base64")
  }), "utf8");
}

function normalizeProducts(value) {
  const allowed = new Set(["monthly_30d", "weekly_100_3d"]);
  const products = Array.isArray(value) ? value.map(String).filter((item) => allowed.has(item)) : [];
  const unique = Array.from(new Set(products));
  if (!unique.length) throw new PublicError("At least one eligible product is required.");
  return unique;
}

function publicInventory(item) {
  const snapshot = item.uploadSnapshot || {};
  return {
    id: item._id,
    displayName: item.displayName,
    status: item.status,
    generation: item.generation,
    eligibleProducts: item.eligibleProducts || [],
    plan: snapshot.plan || "-",
    email: snapshot.email || "-",
    usageAtUpload: snapshot.primaryLeft,
    resetAtUpload: snapshot.resetAt || "-",
    capturedAt: snapshot.capturedAt || item.createdAt,
    deliveryCount: Number(item.deliveryCount || 0),
    lastAllocationId: item.lastAllocationId || null,
    credentialResetRequired: item.credentialResetRequired === true || item.status === "reclaim_pending",
    credentialResetReason: item.credentialResetReason || (item.status === "reclaim_pending" ? "allocation_closed" : ""),
    credentialResetMarkedAt: item.credentialResetMarkedAt || null,
    createdAt: item.createdAt,
    updatedAt: item.updatedAt
  };
}

async function uploadProfile(data, admin) {
  const profile = data.profile && typeof data.profile === "object" ? data.profile : {};
  const profileName = String(profile.profileName || "").trim();
  const authText = String(profile.authText || "");
  const configText = String(profile.configText || "");
  if (!profileName) throw new PublicError("Profile name is required.");
  if (Buffer.byteLength(authText, "utf8") <= MIN_AUTH_BYTES) throw new PublicError("auth.json is smaller than the accepted profile threshold.");
  if (Buffer.byteLength(configText, "utf8") <= MIN_CONFIG_BYTES) throw new PublicError("config.toml is smaller than the accepted profile threshold.");

  const identity = String(profile.accountId || profile.email || profileName).trim();
  const fingerprint = accountHash(identity);
  const duplicate = await db.collection(COLLECTIONS.inventory).where({ accountHash: fingerprint }).limit(1).get();
  if (duplicate.data && duplicate.data[0]) {
    throw new PublicError("This profile already exists in cloud inventory.", "PROFILE_DUPLICATE");
  }

  const profileId = crypto.randomUUID();
  const generation = 1;
  const now = nowIso();
  const eligibleProducts = normalizeProducts(data.eligibleProducts);
  const snapshot = {
    profileName,
    accountId: String(profile.accountId || ""),
    email: String(profile.email || ""),
    plan: String(profile.plan || ""),
    primaryLeft: typeof profile.primaryLeft === "number" ? profile.primaryLeft : null,
    secondaryLeft: typeof profile.secondaryLeft === "number" ? profile.secondaryLeft : null,
    resetAt: String(profile.resetAt || ""),
    profileUpdatedAt: String(profile.profileUpdatedAt || ""),
    authBytes: Buffer.byteLength(authText, "utf8"),
    configBytes: Buffer.byteLength(configText, "utf8"),
    capturedAt: now
  };
  const bundle = encryptBundle({
    schemaVersion: 1,
    profileId,
    generation,
    profileName,
    authText,
    configText,
    uploadSnapshot: snapshot,
    createdAt: now
  });
  const bundleSha256 = crypto.createHash("sha256").update(bundle).digest("hex");
  const cloudPath = `csw_profiles/${profileId}/${generation}.bundle`;
  const upload = await app.uploadFile({ cloudPath, fileContent: bundle });

  const document = {
    _id: profileId,
    displayName: profileName,
    accountHash: fingerprint,
    blobPath: cloudPath,
    fileId: upload.fileID,
    bundleSha256,
    bundleBytes: bundle.length,
    eligibleProducts,
    status: "available",
    generation,
    uploadSnapshot: snapshot,
    deliveryCount: 0,
    lastAllocationId: null,
    credentialResetRequired: false,
    credentialResetReason: "",
    credentialResetMarkedAt: null,
    notes: String(data.notes || "").slice(0, 500),
    uploadedByUid: admin.uid,
    createdAt: now,
    updatedAt: now
  };
  await db.collection(COLLECTIONS.inventory).add(document);
  await audit(admin.uid, "inventory_profile_uploaded", "inventory_profile", profileId, {
    eligibleProducts,
    uploadSnapshot: snapshot,
    bundleSha256,
    bundleBytes: bundle.length
  });
  return { success: true, item: publicInventory(document) };
}

async function listInventory(data, admin) {
  const result = await db.collection(COLLECTIONS.inventory).orderBy("createdAt", "desc").limit(100).get();
  await audit(admin.uid, "inventory_listed", "inventory_profile", "", { count: (result.data || []).length });
  return { success: true, items: (result.data || []).map(publicInventory) };
}

async function moveInventoryPool(data, admin) {
  const profileId = String(data.profileId || "").trim();
  const targetProductId = String(data.targetProductId || "").trim();
  if (!profileId) throw new PublicError("Inventory profile ID is required.");
  if (targetProductId !== "monthly_30d" && targetProductId !== "weekly_100_3d") {
    throw new PublicError("Invalid target inventory pool.");
  }
  const result = await db.collection(COLLECTIONS.inventory).doc(profileId).get();
  const item = result.data && result.data[0];
  if (!item) throw new PublicError("Inventory profile was not found.", "NOT_FOUND");
  const previousProducts = Array.isArray(item.eligibleProducts) ? item.eligibleProducts : [];
  const now = nowIso();
  await db.collection(COLLECTIONS.inventory).doc(profileId).update({
    eligibleProducts: [targetProductId],
    updatedAt: now
  });
  await audit(admin.uid, "inventory_profile_pool_changed", "inventory_profile", profileId, {
    previousProducts,
    targetProductId
  });
  return {
    success: true,
    item: publicInventory({ ...item, eligibleProducts: [targetProductId], updatedAt: now })
  };
}

async function listOrders(data, admin) {
  const result = await db.collection(COLLECTIONS.orders).orderBy("createdAt", "desc").limit(100).get();
  await audit(admin.uid, "orders_listed", "order", "", { count: (result.data || []).length });
  return { success: true, items: result.data || [] };
}

async function listAllocations(data, admin) {
  const result = await db.collection(COLLECTIONS.allocations).orderBy("issuedAt", "desc").limit(100).get();
  await audit(admin.uid, "allocations_listed", "allocation", "", { count: (result.data || []).length });
  return { success: true, items: result.data || [] };
}

async function logout(data, admin) {
  await db.collection(COLLECTIONS.adminSessions).doc(admin.session._id).update({ revoked: true, revokedAt: nowIso() });
  await audit(admin.uid, "admin_logout", "admin_session", admin.session._id);
  return { success: true };
}

exports.main = async (event = {}) => {
  try {
    await ensureCollections();
    const action = String(event.action || "");
    if (action === "login") return await login(event);
    const admin = await assertAdmin(event);
    if (action === "initialize") return { success: true, collections: Object.values(COLLECTIONS) };
    if (action === "listInventory") return await listInventory(event, admin);
    if (action === "uploadProfile") return await uploadProfile(event, admin);
    if (action === "moveInventoryPool") return await moveInventoryPool(event, admin);
    if (action === "listOrders") return await listOrders(event, admin);
    if (action === "listAllocations") return await listAllocations(event, admin);
    if (action === "logout") return await logout(event, admin);
    throw new PublicError("Unknown administrator action.");
  } catch (error) {
    console.error("csw_admin_api failed", error && error.stack || error);
    return {
      success: false,
      code: error && error.code || "INTERNAL_ERROR",
      message: error && error.publicMessage || "Administrator request failed."
    };
  }
};
