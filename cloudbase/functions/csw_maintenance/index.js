const cloudbase = require("@cloudbase/node-sdk");

const ALLOCATIONS = "csw_allocations";
const INVENTORY = "csw_inventory_profiles";
const ORDERS = "csw_orders";
const AUDIT = "csw_audit_events";
const app = cloudbase.init({ env: cloudbase.SYMBOL_CURRENT_ENV });
const db = app.database();

function documentData(result) {
  if (!result || !result.data) return null;
  return Array.isArray(result.data) ? result.data[0] || null : result.data;
}
const command = db.command;

async function markExpired(allocation) {
  const now = new Date().toISOString();
  await db.runTransaction(async transaction => {
    const allocationRef = transaction.collection(ALLOCATIONS).doc(allocation._id);
    const latestResult = await allocationRef.get();
    const latest = documentData(latestResult);
    if (!latest || latest.status !== "active" || Number(latest.expiresAtMs || 0) > Date.now()) return;

    await allocationRef.update({
      status: "expired",
      closedReason: "time_expired",
      closedAt: now,
      updatedAt: now
    });
    await transaction.collection(INVENTORY).doc(latest.profileId).update({
      status: "reclaim_pending",
      credentialResetRequired: true,
      credentialResetReason: "time_expired",
      credentialResetMarkedAt: now,
      updatedAt: now
    });
  });
  await db.collection(AUDIT).add({
    actorType: "system",
    actorUid: "csw_maintenance",
    eventType: "allocation_expired",
    entityType: "allocation",
    entityId: allocation._id,
    metadata: {
      profileId: allocation.profileId,
      orderId: allocation.orderId,
      credentialResetRequired: true,
      credentialResetReason: "time_expired"
    },
    createdAt: now
  });
}

async function releaseExpiredReservation(order) {
  const nowMs = Date.now();
  const now = new Date(nowMs).toISOString();
  let released = false;
  await db.runTransaction(async transaction => {
    const orderRef = transaction.collection(ORDERS).doc(order._id);
    const latestOrderResult = await orderRef.get();
    const latestOrder = documentData(latestOrderResult);
    if (!latestOrder || latestOrder.paymentStatus !== "pending" || latestOrder.fulfillmentStatus !== "reserved") return;
    if (Number(latestOrder.reservationExpiresAtMs || 0) > nowMs || !latestOrder.reservedProfileId) return;
    // Once a cashier trade number exists, keep the inventory reserved until
    // payment reconciliation or an explicit administrator release.
    if (String(latestOrder.tradeNo || "").trim()) return;

    const inventoryRef = transaction.collection(INVENTORY).doc(latestOrder.reservedProfileId);
    const latestInventoryResult = await inventoryRef.get();
    const latestInventory = documentData(latestInventoryResult);
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
      paymentStatus: "expired",
      fulfillmentStatus: "reservation_expired",
      closedReason: "reservation_expired",
      closedAt: now,
      updatedAt: now
    });
    released = true;
  });
  if (!released) return false;
  await db.collection(AUDIT).add({
    actorType: "system",
    actorUid: "csw_maintenance",
    eventType: "inventory_reservation_released",
    entityType: "order",
    entityId: order._id,
    metadata: { profileId: order.reservedProfileId },
    createdAt: now
  });
  return true;
}

exports.main = async () => {
  try {
    const now = Date.now();
    const result = await db.collection(ALLOCATIONS).where({
      status: "active",
      expiresAtMs: command.lte(now)
    }).limit(100).get();
    for (const allocation of result.data || []) await markExpired(allocation);
    const reservationResult = await db.collection(ORDERS).where({ fulfillmentStatus: "reserved" }).limit(100).get();
    let releasedReservationCount = 0;
    for (const order of reservationResult.data || []) {
      if (Number(order.reservationExpiresAtMs || 0) <= now && await releaseExpiredReservation(order)) {
        releasedReservationCount += 1;
      }
    }
    return {
      success: true,
      expiredCount: (result.data || []).length,
      releasedReservationCount
    };
  } catch (error) {
    console.error("csw_maintenance failed", error && error.stack || error);
    return { success: false, message: "Maintenance task failed." };
  }
};
