var profiles = [];
var appVersion = "…";

var selectedName = "";
var selectedVisualName = "";
var selectedClearTimer = null;
var sideMode = "";
var sideOpen = false;
var configViewMode = "auth";
var layoutMode = "single";
var loginWatchTimer = null;
var loginWatchRunId = 0;
var loginStableSignature = "";
var loginStableCount = 0;
var initialUsageRefreshed = false;
var usageCache = {};
var usageRefreshTimer = null;
var usageRefreshRunning = false;
var usageRefreshRunId = 0;
var fiveHourRequestRunning = false;
var profileSwitchRunning = false;
var manualFiveHourConfirmationPending = false;
var autoFiveHourStorageKey = "codex-switch-auto-five-hour-enabled";
var autoFiveHourStateKey = "codex-switch-auto-five-hour-state";
var autoFiveHourEnabled = loadBooleanSetting(autoFiveHourStorageKey, false);
var autoFiveHourScheduler = null;
var autoFiveHourDrainTimer = null;
var baseWindowHeight = 740;
var statusExtraHeight = 0;
var statusIdleTimer = null;
var statusLastOperationAt = 0;
var statusIdleDelayMs = 5 * 1000;
var fiveHourStatusText = null;
var fiveHourStatusRestoreTimer = null;
var fiveHourConfirmedProfiles = new Set();
var latestTiboPost = {
  text: "Bullish",
  url: "https://x.com/thsottiaux/status/2094939684015317363",
  createdAt: "Sep 2, 2026 · 12:05 AM UTC"
};
var tiboRefreshTimer = null;
var tiboRefreshIntervalMs = 5 * 60 * 1000;
var tiboTickerCharactersPerSecond = 5;
var codexResetStatus = "unknown";
var resetPostAt = 0;
var resetPostStorageKey = "codex-switch-reset-post-at";
var mockProfileCount = getMockProfileCount();
var isMockMode = mockProfileCount > 0;
var showEmails = false;
var forceSingleColumn = false;
var profileOrderStorageKey = "codex-switch-profile-order";
var floatingBarStorageKey = "codex-switch-floating-bar-enabled";
var profileSnapshotStorageKey = "codex-switch-profile-snapshot";
var usageCacheStorageKey = "codex-switch-usage-cache";
var latestTiboPostStorageKey = "codex-switch-latest-tibo-post";
var showTiboPostStorageKey = "codex-switch-show-tibo-post";
var pendingOrderStorageKey = "codex-switch-pending-order";
var storeNoticeExpandedStorageKey = "codex-switch-store-notice-expanded";
var floatingBarEnabled = loadFloatingBarEnabled();
var showTiboPost = loadBooleanSetting(showTiboPostStorageKey, true);
var dragState = null;
var suppressNextCardClick = false;
var confirmDialogOpen = false;
var paymentSession = null;
var deliverySyncRunning = false;
var deliveryPollTimer = null;
var deliveryPollStartedAt = 0;
var paymentReconcileRunning = false;
var syncLocalProjectsEnabled = false;
var syncLocalProjectsSettingBusy = true;
var languageStorageKey = "codex-switch-language";
var currentLanguage = loadStoredLanguage();
var selectedStoreProduct = "monthly";
var storeInventoryLoaded = false;
var storePageHtml = "";
var storeNoticeExpanded = loadBooleanSetting(storeNoticeExpandedStorageKey, false);
var storeCatalogProducts = [];
var storePanelResizeObserver = null;
var cloudbaseState = { configured: false, envId: "", configPath: "" };
var cloudbaseIdentity = null;
var adminUnlocked = false;
var adminToken = "";
var adminSessionExpiresAt = "";
var adminInventory = [];
var adminVersionHoldTimer = null;
var selectedAdminInventoryId = "";
var adminInventoryFilter = "monthly";
var singleWindowWidth = 330;
var doubleWindowWidth = 680;
var adminPreviewWidth = 500;
var adminWindowWidth = doubleWindowWidth + adminPreviewWidth;

var storeProducts = {
  monthly: {
    id: "monthly",
    cloudProductId: "monthly_30d",
    price: "&yen;62",
    priceNumber: 62,
    goodsKey: "bpx5pe",
    itemUrl: "https://catfk.com/item/bpx5pe",
    availableCount: 0
  },
  weekly: {
    id: "weekly",
    cloudProductId: "weekly_100_3d",
    price: "&yen;20",
    priceNumber: 20,
    goodsKey: "t133lf",
    itemUrl: "https://catfk.com/item/t133lf",
    availableCount: 0
  }
};

var i18n = {
  en: {
    eyebrow: "Local profile switcher",
    profiles: "Profiles",
    profileHint: "Choose an account snapshot to deploy.",
    refresh: "Refresh",
    activate: "Activate",
    newProfile: "New profile",
    delete: "Delete",
    store: "Store",
    more: "More",
    singleColumn: "Single column",
    restoreTwoColumns: "Restore two columns",
    ready: "Ready",
    resetStatusConfirmed: "Tweet within the past 24h contains reset",

    resetStatusUnknown: "Codex reset: no current signal",
    abort: "Abort",
    config: "Config",
    auth: "Auth",
    plan: "Plan",
    email: "Email",
    reset: "Reset",
    startFiveHour: "Fix 5h quota reset time",
    startFiveHourConfirm: "Use this profile ({profile}) to run one short Codex task in an isolated CLI session? This uses a small amount of quota and leaves the desktop task running.",
    startFiveHourRunning: "Starting the 5-hour window...",
    startFiveHourConfirmed: "5-hour window confirmed",
    startFiveHourAlreadyActive: "5-hour window already active",
    startFiveHourUnconfirmed: "Request completed; refresh time is not confirmed yet",
    startFiveHourChatArchived: "Probe chat archived",
    startFiveHourChatReused: "Probe chat could not be removed; it will be reused next time",
    primaryLeft: "Primary left",
    usageLeftCombined: "Usage left (5h | 7d)",
    saveCurrent: "Save Current",
    autoFiveHour: "Auto Fix 5h",
    autoFiveHourHint: "While this app is running, check every minute for all accounts, including the current account. Queue short tasks after two readings confirm a rolling 5h reset time. Uses a small amount of quota; failed attempts retry with backoff. Turning off clears waiting tasks; a running task finishes normally.",
    syncLocalProjects: "Sync Local Projects",
    syncLocalProjectsHint: "When switching accounts, restore local projects if the destination is empty and recover missing local chats in matching projects.",
    syncLocalProjectsSaveFailed: "Could not save the local project sync setting.",
    emailShow: "Email: show",
    emailHide: "Email: hide",
    languageButton: "Lang: en",
    codexDir: "Codex Dir",
    dataDir: "Data Dir",
    openApp: "Open ChatGPT/Codex",
    hardKill: "Hard Kill ChatGPT/Codex",
    floatingBar: "Floating Bar",
    showTiboRecentPost: "Tibo's Post",
    on: "ON",
    off: "OFF",
    monthlyAccess: "Store",
    monthlyPass: "Codex Monthly Pass",
    recommended: "Recommended",
    perMonth: "/ month",
    monthlyFeatureAccess: "30-day access",
    monthlyFeaturePlus: "Official full-price Plus account",
    weeklyPass: "Codex one-time weekly quota boost",
    weeklyPriceUnit: "/ 100% weekly quota",
    weeklyFeatureAccess: "One-time 100% quota, valid for 7 days",
    purchasedMonthly: "Monthly access (In-app)",
    purchasedWeekly: "Weekly quota (In-app)",
    checkingStock: "Checking stock",
    inStock: "In stock",
    soldOut: "Sold out",
    storeNotice: "Notice: Accounts are official full-price subscriptions with the Codex quota portion separated. A fixed US IP node is required when using the purchased account (California is preferred). Risk controls or account bans caused by VPN/IP switching are not covered by after-sales support. Official subscription accounts only: not daily disposable, not proxy-transit, and reverse proxying is not allowed.",
    featureSwitchBackup: "Profile snapshot backup before switching",
    featureUsage: "Usage percentage and reset-time display",
    featureFloating: "Floating status bar support",
    paymentSummary: "Payment summary",
    secureCheckout: "Secure checkout",
    billingPeriod: "Billing period",
    thirtyDays: "30 days",
    taxesFees: "Taxes / fees",
    includedLater: "Included later",
    totalDue: "Amount due",
    proceedPayment: "Proceed to payment",
    scanToPay: "Scan to pay",
    creatingOrder: "Creating order...",
    fetchingQr: "Fetching QR code...",
    paymentConfirmed: "Payment confirmed. Installing profile...",
    deliveryComplete: "Purchased profile installed",
    haveRedemptionCode: "I have a code",
    cancel: "Cancel",
    ok: "OK",
    close: "Close",
    minimize: "Minimize",
    configContent: "Config content",
    adminToolbox: "Admin Toolbox",
    adminPasswordTitle: "Administrator Access",
    adminPasswordMessage: "Enter the administrator password to unlock cloud inventory tools.",
    adminPasswordPlaceholder: "Administrator password",
    uploadSelected: "Upload Selected Profile",
    refreshInventory: "Refresh Inventory",
    adminLogout: "Admin Logout",
    transferPool: "Move to other pool",
    chooseUploadPool: "Choose inventory pool",
    chooseUploadPoolMessage: "Choose the pool for the selected local profile.",
    eligibleProduct: "Eligible product",
    cloudInventory: "Cloud inventory",
    inventoryPreview: "Inventory preview",
    inventoryPreviewHint: "Select a cloud profile for toolbox actions.",
    selectedInventory: "Selected cloud profile",
    noInventorySelection: "None selected",
    credentialReset: "Credential reset",
    yes: "Yes",
    no: "No",
    filterWeekly: "Weekly",
    filterMonthly: "Monthly",
    inventoryEmpty: "No cloud profiles",
    cloudNotConfigured: "CloudBase is not configured",
    newProfileLoginTitle: "New Profile Login",
    newProfileLoginMessage: "Sign in to ChatGPT first. Your current profile remains active until the web session is confirmed. Cancelling later will restore it automatically.",
    loginWaiting: "Waiting for ChatGPT sign-in. You can cancel at any time."
  },
  "zh-cn": {
    eyebrow: "账号切换器",
    profiles: "配置档案",
    profileHint: "选择一个账号卡片进行切换。",
    refresh: "刷新",
    activate: "启用",
    newProfile: "新建档案",
    delete: "删除",
    store: "商店",
    more: "更多",
    singleColumn: "单列",
    restoreTwoColumns: "恢复双列",
    ready: "就绪",
    resetStatusConfirmed: "近 24 小时推文包含 reset",

    resetStatusUnknown: "Codex 重置：暂无信号",
    abort: "放弃",
    config: "配置",
    auth: "Auth",
    plan: "套餐",
    email: "邮箱",
    reset: "重置",
    startFiveHour: "固定5h额度刷新时间",
    startFiveHourConfirm: "使用此档案（{profile}）在独立的 CLI 会话中运行一个简短任务，以固定 5 小时额度刷新时间？这会消耗少量额度，当前桌面任务会继续运行。",
    startFiveHourRunning: "正在启动 5 小时窗口…",
    startFiveHourConfirmed: "已确认 5 小时窗口",
    startFiveHourAlreadyActive: "5 小时窗口已经启动",
    startFiveHourUnconfirmed: "请求已完成，重置时间尚未确认",
    startFiveHourChatArchived: "测试聊天已归档",
    startFiveHourChatReused: "测试聊天暂时无法删除或归档，下次将复用它",
    primaryLeft: "主额度剩余",
    usageLeftCombined: "额度剩余 (5h | 7d)",
    saveCurrent: "保存当前",
    autoFiveHour: "自动固定 5h",
    autoFiveHourHint: "程序运行期间每分钟检查所有账号（包含当前账号）；连续两次确认时间仍在滚动时，排队运行简单任务，会消耗少量额度。失败后间隔重试。关闭后清空等待队列，正在执行的任务正常结束。",
    syncLocalProjects: "同步本地项目",
    syncLocalProjectsHint: "切换账号时，若目标账号没有本地项目则恢复项目，并在相同项目下补齐旧的本地聊天。",
    syncLocalProjectsSaveFailed: "无法保存本地项目同步设置。",
    emailShow: "邮箱: 显示",
    emailHide: "邮箱: 隐藏",
    languageButton: "语言：简中",
    codexDir: "Codex 目录",
    dataDir: "数据目录",
    openApp: "打开 ChatGPT/Codex",
    hardKill: "强制结束 ChatGPT/Codex",
    floatingBar: "悬浮条",
    showTiboRecentPost: "Tibo社媒",
    on: "开",
    off: "关",
    monthlyAccess: "商店",
    monthlyPass: "Codex 月度通行证",
    recommended: "推荐",
    perMonth: "/ 月",
    monthlyFeatureAccess: "30天使用权限",
    monthlyFeaturePlus: "官方全价订阅Plus账号",
    weeklyPass: "Codex单次周额度速蹬",
    weeklyPriceUnit: "/ 100%周额度",
    weeklyFeatureAccess: "单次100%额度，有效期7天",
    purchasedMonthly: "月度通行证 (内购)",
    purchasedWeekly: "单次周额度 (内购)",
    checkingStock: "查询库存中",
    inStock: "有货",
    soldOut: "售罄",
    storeNotice: "注意事项：账号为全价官方订阅后剥离出Codex额度部分。购买账号使用需要US固定IP节点（最好是加利福尼亚州节点），梯子和IP切换问题引起的风控和封号无售后。官方订阅，非日抛、非中转，不可反代。",
    featureSwitchBackup: "切换前自动备份账号快照",
    featureUsage: "显示额度百分比与重置时间",
    featureFloating: "支持悬浮状态条",
    paymentSummary: "支付信息",
    secureCheckout: "安全结算",
    billingPeriod: "有效期",
    thirtyDays: "30 天",
    taxesFees: "税费",
    includedLater: "后续包含",
    totalDue: "应付",
    proceedPayment: "继续支付",
    scanToPay: "扫码支付",
    creatingOrder: "正在建立订单...",
    fetchingQr: "正在获取二维码...",
    paymentConfirmed: "付款已确认，正在安装档案...",
    deliveryComplete: "购买的档案已安装",
    haveRedemptionCode: "我有兑换码",
    cancel: "取消",
    ok: "确定",
    close: "关闭",
    minimize: "最小化",
    configContent: "配置内容",
    adminToolbox: "管理员工具箱",
    adminPasswordTitle: "管理员验证",
    adminPasswordMessage: "输入管理员密码以解锁云端库存工具。",
    adminPasswordPlaceholder: "管理员密码",
    uploadSelected: "上传选中档案",
    refreshInventory: "刷新库存",
    adminLogout: "退出管理员模式",
    transferPool: "转到对池",
    chooseUploadPool: "选择库存池",
    chooseUploadPoolMessage: "选择当前本地档案要上传到的库存池。",
    eligibleProduct: "适用商品",
    cloudInventory: "云端库存",
    inventoryPreview: "库存预览",
    inventoryPreviewHint: "选择云端档案，供工具箱执行操作。",
    selectedInventory: "已选云端档案",
    noInventorySelection: "尚未选择",
    credentialReset: "需重置密钥",
    yes: "是",
    no: "否",
    filterWeekly: "周",
    filterMonthly: "月",
    inventoryEmpty: "暂无云端档案",
    cloudNotConfigured: "CloudBase 尚未配置",
    newProfileLoginTitle: "新建配置档案",
    newProfileLoginMessage: "请先登录 ChatGPT。网页会话确认前不会改动当前档案；之后如取消或授权失败，程序会自动恢复原档案。",
    loginWaiting: "正在等待 ChatGPT 登录，可随时放弃。"
  }
};

var els = {
  appShell: document.querySelector(".app-shell"),
  workspace: document.querySelector(".workspace"),
  profileList: document.getElementById("profileList"),
  profileHint: document.getElementById("profileHint"),
  statusbar: document.getElementById("statusbar"),
  resetStatusIndicator: document.getElementById("resetStatusIndicator"),
  statusText: document.getElementById("statusText"),
  abortLoginWatchBtn: document.getElementById("abortLoginWatchBtn"),
  columnToggleBtn: document.getElementById("columnToggleBtn"),
  switchBtn: document.getElementById("switchBtn"),
  deleteBtn: document.getElementById("deleteBtn"),
  sidePanel: document.getElementById("sidePanel"),
  sideTitle: document.getElementById("sideTitle"),
  sideControls: document.getElementById("sideControls"),
  sideBody: document.getElementById("sideBody"),
  closeSideBtn: document.getElementById("closeSideBtn"),
  closeSideIcon: document.getElementById("closeSideIcon"),
  adminInventoryPanel: document.getElementById("adminInventoryPanel"),
  adminInventoryTitle: document.getElementById("adminInventoryTitle"),
  adminInventoryCount: document.getElementById("adminInventoryCount"),
  adminInventoryGrid: document.getElementById("adminInventoryGrid"),
  adminInventoryRefreshBtn: document.getElementById("adminInventoryRefreshBtn"),
  adminFilterWeeklyBtn: document.getElementById("adminFilterWeeklyBtn"),
  adminFilterMonthlyBtn: document.getElementById("adminFilterMonthlyBtn"),
  confirmLayer: document.getElementById("confirmLayer"),
  confirmTitle: document.getElementById("confirmTitle"),
  confirmMessage: document.getElementById("confirmMessage"),
  confirmInput: document.getElementById("confirmInput"),
  confirmChoice: document.getElementById("confirmChoice"),
  confirmCancelBtn: document.getElementById("confirmCancelBtn"),
  confirmOkBtn: document.getElementById("confirmOkBtn"),
  paymentLayer: document.getElementById("paymentLayer"),
  paymentSubtitle: document.getElementById("paymentSubtitle"),
  paymentBody: document.getElementById("paymentBody"),
  paymentCloseBtn: document.getElementById("paymentCloseBtn"),
  paymentCancelBtn: document.getElementById("paymentCancelBtn"),
  paymentRedeemCodeBtn: document.getElementById("paymentRedeemCodeBtn"),
  windowDragMask: document.getElementById("windowDragMask"),
  toast: document.getElementById("toast")
};

function getSelectedProfile() {
  if (profiles.length === 0) return null;
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name === selectedName) return profiles[i];
  }
  return profiles[0];
}

function getProfileByName(name) {
  if (!name) return null;
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name === name) return profiles[i];
  }
  return null;
}

function normalizeLanguage(value) {
  var text = String(value || "").toLowerCase();
  return text === "zh" || text === "zh-cn" || text === "zh_cn" || text === "cn" ? "zh-cn" : "en";
}

function loadStoredLanguage() {
  try {
    return normalizeLanguage(window.localStorage.getItem(languageStorageKey));
  } catch (error) {
    return "en";
  }
}

function saveStoredLanguage(value) {
  try {
    window.localStorage.setItem(languageStorageKey, normalizeLanguage(value));
  } catch (error) {
    console.warn("save language failed", error);
  }
}

function t(key) {
  var table = i18n[currentLanguage] || i18n.en;
  return table[key] || i18n.en[key] || key;
}

function setElementTitle(button, title) {
  if (!button) return;
  button.title = title;
  button.setAttribute("aria-label", title);
}

function applyStaticText() {
  var eyebrow = document.querySelector(".eyebrow");
  var profilesTitle = document.querySelector(".section-heading h2");
  if (eyebrow) eyebrow.textContent = t("eyebrow");
  if (profilesTitle) profilesTitle.textContent = t("profiles");
  els.profileHint.textContent = t("profileHint");
  els.abortLoginWatchBtn.textContent = t("abort");
  setElementTitle(document.getElementById("refreshBtn"), t("refresh"));
  setElementTitle(document.getElementById("switchBtn"), t("activate"));
  setElementTitle(document.getElementById("loginBtn"), t("newProfile"));
  setElementTitle(document.getElementById("deleteBtn"), t("delete"));
  setElementTitle(document.getElementById("storeBtn"), t("store"));
  setElementTitle(document.getElementById("moreBtn"), t("more"));
  setElementTitle(document.getElementById("minimizeWindowBtn"), t("minimize"));
  setElementTitle(document.getElementById("closeWindowBtn"), t("close"));
  els.confirmCancelBtn.textContent = t("cancel");
  els.confirmOkBtn.textContent = t("ok");
  els.paymentRedeemCodeBtn.textContent = t("haveRedemptionCode");
  els.paymentCancelBtn.textContent = t("cancel");
  document.getElementById("paymentTitle").textContent = t("scanToPay");
  renderCodexResetStatus(codexResetStatus);
  if (els.statusText.textContent === "Ready" || els.statusText.textContent === "就绪") {
    setStatus(t("ready"));
  }
}

function applyLanguage(language) {
  currentLanguage = normalizeLanguage(language);
  saveStoredLanguage(currentLanguage);
  applyStaticText();
  renderProfiles();
  if (sideOpen) {
    if (sideMode === "more") renderMore();
    if (sideMode === "store") renderStore();
    if (sideMode === "config") renderConfig();
  }
}

function loadAppSettings() {
  var invoke = getTauriInvoke();
  if (!invoke) {
    applyLanguage(currentLanguage);
    return Promise.resolve({ language: currentLanguage });
  }
  return invoke("get_app_settings", {}).then(function (settings) {
    syncLocalProjectsEnabled = Boolean(settings && settings.syncLocalProjects === true);
    syncLocalProjectsSettingBusy = false;
    applyLanguage(settings && settings.language ? settings.language : currentLanguage);
    return settings;
  }).catch(function (error) {
    console.warn("get_app_settings failed", error);
    syncLocalProjectsSettingBusy = false;
    applyLanguage(currentLanguage);
  });
}

function loadAppVersion() {
  var invoke = getTauriInvoke();
  if (!invoke) return Promise.resolve(appVersion);
  return invoke("get_app_version", {}).then(function (version) {
    appVersion = String(version || appVersion);
    if (sideOpen && sideMode === "more") renderMore();
    return appVersion;
  }).catch(function (error) {
    console.warn("get_app_version failed", error);
    return appVersion;
  });
}

function setAppLanguage(language) {
  var next = normalizeLanguage(language);
  var invoke = getTauriInvoke();
  applyLanguage(next);
  if (!invoke) return Promise.resolve();
  return invoke("set_app_language", { language: next }).catch(function (error) {
    console.warn("set_app_language failed", error);
  });
}

function loadBooleanSetting(key, fallback) {
  try {
    var value = window.localStorage.getItem(key);
    if (value === "true") return true;
    if (value === "false") return false;
  } catch (error) {
    console.warn("load setting failed", key, error);
  }
  return fallback;
}

function saveBooleanSetting(key, value) {
  try {
    window.localStorage.setItem(key, value ? "true" : "false");
  } catch (error) {
    console.warn("save setting failed", key, error);
  }
}

function loadFloatingBarEnabled() {
  try {
    var value = window.localStorage.getItem(floatingBarStorageKey);
    if (value === "true" || value === "false") return value === "true";
    return window.localStorage.getItem("codex-switch-display-mode") === "floating";
  } catch (error) {
    console.warn("load floating bar setting failed", error);
  }
  return false;
}

function saveFloatingBarEnabled(value) {
  saveBooleanSetting(floatingBarStorageKey, value);
}

function loadProfileOrder() {
  try {
    var text = window.localStorage.getItem(profileOrderStorageKey);
    var order = text ? JSON.parse(text) : [];
    return Array.isArray(order) ? order : [];
  } catch (error) {
    return [];
  }
}

function saveProfileOrder() {
  try {
    window.localStorage.setItem(profileOrderStorageKey, JSON.stringify(profiles.map(function (profile) {
      return profile.name;
    })));
  } catch (error) {
    console.warn("save profile order failed", error);
  }
}

function loadUsageCache() {
  try {
    var text = window.localStorage.getItem(usageCacheStorageKey);
    var value = text ? JSON.parse(text) : {};
    usageCache = value && typeof value === "object" && !Array.isArray(value) ? value : {};
  } catch (error) {
    usageCache = {};
  }
}

function saveUsageCache() {
  try {
    window.localStorage.setItem(usageCacheStorageKey, JSON.stringify(usageCache));
  } catch (error) {
    console.warn("save usage cache failed", error);
  }
}

function profileCardSnapshot(profile) {
  return {
    name: profile.name,
    active: !!profile.active,
    plan: profile.plan || "-",
    email: profile.email || "-",
    accountId: profile.accountId || profile.account_id || "-",
    primaryLeft: typeof profile.primaryLeft === "number" ? profile.primaryLeft : null,
    secondaryLeft: typeof profile.secondaryLeft === "number" ? profile.secondaryLeft : null,
    expiresAt: profile.expiresAt || "",
    reset: profile.reset || "-",
    updated: profile.updated || "-",
    authSize: profile.authSize || "-",
    configSize: profile.configSize || "-",
    source: profile.source || "",
    productId: profile.productId || "",
    orderId: profile.orderId || "",
    allocationId: profile.allocationId || "",
    entitlementExpiresAt: profile.entitlementExpiresAt || "",
    authText: "",
    configText: ""
  };
}

function saveProfileSnapshot() {
  if (isMockMode) return;
  try {
    window.localStorage.setItem(profileSnapshotStorageKey, JSON.stringify({
      savedAt: Date.now(),
      profiles: profiles.map(profileCardSnapshot)
    }));
  } catch (error) {
    console.warn("save profile snapshot failed", error);
  }
}

function loadProfileSnapshot() {
  if (isMockMode) return false;
  try {
    var text = window.localStorage.getItem(profileSnapshotStorageKey);
    if (!text) return false;
    var value = JSON.parse(text);
    if (!value || !Array.isArray(value.profiles) || value.profiles.length === 0) return false;
    applySnapshot({ profiles: value.profiles }, "Cached profile snapshot loaded.");
    return true;
  } catch (error) {
    console.warn("load profile snapshot failed", error);
    return false;
  }
}

function applySavedProfileOrder() {
  var order = loadProfileOrder();
  if (order.length === 0 || profiles.length < 2) return;
  var rank = {};
  for (var i = 0; i < order.length; i += 1) {
    rank[order[i]] = i;
  }
  profiles.sort(function (a, b) {
    var ar = Object.prototype.hasOwnProperty.call(rank, a.name) ? rank[a.name] : Number.MAX_SAFE_INTEGER;
    var br = Object.prototype.hasOwnProperty.call(rank, b.name) ? rank[b.name] : Number.MAX_SAFE_INTEGER;
    if (ar !== br) return ar - br;
    return a.name.localeCompare(b.name);
  });
}

function updateActionButtons() {
  var visualProfile = getProfileByName(selectedVisualName);
  var canDelete = !!visualProfile;
  var canSwitch = !!visualProfile && !visualProfile.active;
  els.switchBtn.disabled = !canSwitch;
  els.deleteBtn.disabled = !canDelete;
}

function updateProfileHintVisibility() {
  var hidden = layoutMode === "single" || (layoutMode === "double" && sideOpen);
  els.profileHint.classList.toggle("hint-hidden", hidden);
  els.profileHint.setAttribute("aria-hidden", hidden ? "true" : "false");
}

function setStatus(text) {
  if (fiveHourStatusText !== null) return;
  statusLastOperationAt = Date.now();
  renderOperationStatus(text);
  console.log("[Codex Switch] " + text);
  scheduleTiboPostRestore();
}

function renderOperationStatus(text) {
  if (fiveHourStatusText !== null) return;
  var content = document.createElement("span");
  content.className = "status-content";
  content.textContent = text;
  els.statusText.replaceChildren(content);
  els.statusbar.classList.remove("ticker-mode");
  els.statusbar.classList.add("operation-mode");
  els.statusText.setAttribute("aria-label", text);
  updateStatusHeight();
}

function formatTiboPostTime(value) {
  if (!value) return "";
  var text = String(value).trim();
  var nitterTime = text.match(/^([A-Za-z]{3})\s+(\d{1,2}),\s+(\d{4})\s+·\s+(\d{1,2}):(\d{2})\s+(AM|PM)\s+UTC$/i);
  var date;
  if (nitterTime) {
    var months = { Jan: 0, Feb: 1, Mar: 2, Apr: 3, May: 4, Jun: 5, Jul: 6, Aug: 7, Sep: 8, Oct: 9, Nov: 10, Dec: 11 };
    var month = months[nitterTime[1].slice(0, 1).toUpperCase() + nitterTime[1].slice(1, 3).toLowerCase()];
    var hour = Number(nitterTime[4]) % 12 + (nitterTime[6].toUpperCase() === "PM" ? 12 : 0);
    date = new Date(Date.UTC(Number(nitterTime[3]), month, Number(nitterTime[2]), hour, Number(nitterTime[5])));
  } else {
    date = new Date(text);
  }
  if (Number.isNaN(date.getTime())) return "";
  function pad(number) {
    return String(number).padStart(2, "0");
  }
  return pad(date.getMonth() + 1) + "-" + pad(date.getDate()) + " " + pad(date.getHours()) + ":" + pad(date.getMinutes());
}

function getTiboTickerDuration(text) {
  var characterCount = Array.from(String(text || "")).length;
  var gapCharacterEquivalent = 6;
  return (characterCount + gapCharacterEquivalent) / tiboTickerCharactersPerSecond;
}

function renderTiboPost(post) {
  var timestamp = formatTiboPostTime(post.createdAt);
  var bodyText = post.text + (timestamp ? "（" + timestamp + "）" : "");
  renderStatusTicker("Tibo：", bodyText);
}

function renderStatusTicker(labelText, bodyText) {
  var fullText = labelText + bodyText;
  var content = document.createElement("span");
  content.className = "status-content";
  var label = document.createElement("span");
  label.className = "status-label";
  label.textContent = labelText;
  var viewport = document.createElement("span");
  viewport.className = "status-ticker-viewport";
  var track = document.createElement("span");
  track.className = "status-ticker-track";
  var first = document.createElement("span");
  first.className = "status-ticker-copy";
  first.textContent = bodyText;
  track.appendChild(first);
  viewport.appendChild(track);
  content.appendChild(label);
  content.appendChild(viewport);
  els.statusText.replaceChildren(content);
  els.statusbar.classList.remove("operation-mode");
  els.statusText.setAttribute("aria-label", fullText);

  var shouldScroll = label.offsetWidth + Math.ceil(first.getBoundingClientRect().width) > els.statusText.clientWidth;
  els.statusbar.classList.toggle("ticker-mode", shouldScroll);
  if (shouldScroll) {
    track.style.setProperty("--status-ticker-duration", getTiboTickerDuration(bodyText) + "s");
    var duplicate = document.createElement("span");
    duplicate.className = "status-ticker-copy";
    duplicate.textContent = bodyText;
    duplicate.setAttribute("aria-hidden", "true");
    track.appendChild(duplicate);
  }
  updateStatusHeight();
}

function showLatestTiboPost() {
  if (fiveHourStatusText !== null) return;
  if (!showTiboPost || !latestTiboPost || !latestTiboPost.text) return;
  renderTiboPost(latestTiboPost);
}

function statusIsIdle() {
  return fiveHourStatusText === null && showTiboPost && (!statusLastOperationAt || Date.now() - statusLastOperationAt >= statusIdleDelayMs);
}

function showLatestTiboPostIfIdle() {
  if (statusIsIdle()) showLatestTiboPost();
}

function scheduleTiboPostRestore() {
  if (statusIdleTimer) window.clearTimeout(statusIdleTimer);
  statusIdleTimer = null;
  if (!showTiboPost) return;
  var remaining = Math.max(0, statusIdleDelayMs - (Date.now() - statusLastOperationAt));
  statusIdleTimer = window.setTimeout(function () {
    statusIdleTimer = null;
    showLatestTiboPostIfIdle();
  }, remaining);
}

function loadCachedTiboPost() {
  try {
    var cached = JSON.parse(window.localStorage.getItem(latestTiboPostStorageKey) || "null");
    if (cached && cached.text) latestTiboPost = cached;
  } catch (error) {
    console.warn("load cached Tibo post failed", error);
  }
}

function saveCachedTiboPost(post) {
  try {
    window.localStorage.setItem(latestTiboPostStorageKey, JSON.stringify(post));
  } catch (error) {
    console.warn("save cached Tibo post failed", error);
  }
}

function renderCodexResetStatus(status) {
  var normalized = String(status || "").trim().toLowerCase();
  var visualState = normalized === "confirmed" ? "confirmed" : "unknown";
  var labelKey = visualState === "confirmed" ? "resetStatusConfirmed" : "resetStatusUnknown";
  codexResetStatus = visualState;
  els.resetStatusIndicator.className = "reset-status-indicator reset-status-" + visualState;
  var label = t(labelKey);
  if (visualState === "confirmed") label += currentLanguage === "zh-cn" ? "；长按打开 Tibo 推文列表" : "; hold to open Tibo’s timeline";
  els.resetStatusIndicator.title = label;
  els.resetStatusIndicator.setAttribute("aria-label", label);
}

function refreshResetIndicator() {
  var now = Date.now() / 1000;
  var active = Number.isFinite(resetPostAt) && resetPostAt <= now && resetPostAt > now - 86400;
  renderCodexResetStatus(active ? "confirmed" : "unknown");
}

function updateResetIndicatorFromTimeline(timeline) {
  var timestamp = Number(timeline && timeline.resetPostAt);
  var now = Date.now() / 1000;
  if (Number.isFinite(timestamp) && timestamp > now - 86400 && timestamp <= now && timestamp > resetPostAt) {
    resetPostAt = timestamp;
    try { window.localStorage.setItem(resetPostStorageKey, String(resetPostAt)); } catch (_) {}
  }
  refreshResetIndicator();
}

function startResetHistoryIndicator() {
  try { resetPostAt = Number(window.localStorage.getItem(resetPostStorageKey)) || 0; } catch (_) {}
  if (!Number.isFinite(resetPostAt) || resetPostAt > Date.now() / 1000) resetPostAt = 0;
  refreshResetIndicator();
  window.setInterval(refreshResetIndicator, 30000);
}

function refreshLatestTiboPost() {
  var invoke = getTauriInvoke();
  if (!invoke) return Promise.resolve();
  return invoke("get_tibo_timeline").then(function (timeline) {
    updateResetIndicatorFromTimeline(timeline);
    var post = timeline && timeline.latestPost;
    if (!post || !post.text) {
      return;
    }
    latestTiboPost = post;
    saveCachedTiboPost(post);
    showLatestTiboPostIfIdle();
  }).catch(function (error) {
    refreshResetIndicator();
    console.warn("get_tibo_timeline failed; keeping cached post and unexpired reset signal", error);
  });
}

function startTiboPostTicker() {
  loadCachedTiboPost();
  if (!showTiboPost) renderOperationStatus(t("ready"));
  showLatestTiboPostIfIdle();
  refreshLatestTiboPost();
  if (tiboRefreshTimer) window.clearInterval(tiboRefreshTimer);
  tiboRefreshTimer = window.setInterval(refreshLatestTiboPost, tiboRefreshIntervalMs);
}

function setTiboPostVisibility(enabled) {
  showTiboPost = !!enabled;
  saveBooleanSetting(showTiboPostStorageKey, showTiboPost);
  if (statusIdleTimer) {
    window.clearTimeout(statusIdleTimer);
    statusIdleTimer = null;
  }
  if (showTiboPost) {
    statusLastOperationAt = 0;
    showLatestTiboPost();
    refreshLatestTiboPost();
  } else {
    renderOperationStatus(t("ready"));
  }
}

function setupResetIndicatorLongPress() {
  var indicator = els.resetStatusIndicator;
  var timer = null;
  var origin = null;
  function cancel() {
    if (timer !== null) window.clearTimeout(timer);
    timer = null;
    origin = null;
  }
  function begin() {
    cancel();
    if (codexResetStatus !== "confirmed") return;
    timer = window.setTimeout(function () {
      timer = null;
      if (codexResetStatus !== "confirmed") return;
      var invoke = getTauriInvoke();
      if (invoke) invoke("open_tibo_timeline").catch(function (error) { setStatus(String(error)); });
    }, 700);
  }
  indicator.addEventListener("pointerdown", function (event) {
    if (event.button !== 0 || !event.isPrimary) return;
    event.preventDefault();
    event.stopPropagation();
    begin();
    origin = { x: event.clientX, y: event.clientY };
    indicator.setPointerCapture(event.pointerId);
  });
  indicator.addEventListener("pointermove", function (event) {
    if (origin && Math.hypot(event.clientX - origin.x, event.clientY - origin.y) > 8) cancel();
  });
  ["pointerup", "pointercancel", "lostpointercapture"].forEach(function (name) { indicator.addEventListener(name, cancel); });
  indicator.addEventListener("keydown", function (event) {
    if ((event.key === " " || event.key === "Enter") && !event.repeat) { event.preventDefault(); begin(); }
  });
  indicator.addEventListener("keyup", function (event) {
    if (event.key === " " || event.key === "Enter") { event.preventDefault(); cancel(); }
  });
  indicator.addEventListener("blur", cancel);
  window.addEventListener("blur", cancel);
}

function updateStatusHeight() {
  window.requestAnimationFrame(function () {
    var style = window.getComputedStyle(els.statusbar);
    var lineHeight = parseFloat(style.lineHeight) || 18;
    var verticalBox =
      (parseFloat(style.paddingTop) || 0) +
      (parseFloat(style.paddingBottom) || 0) +
      (parseFloat(style.borderTopWidth) || 0) +
      (parseFloat(style.borderBottomWidth) || 0);
    var singleLineHeight = Math.ceil(lineHeight + verticalBox);
    var twoLineHeight = Math.ceil(lineHeight * 2 + verticalBox);
    var wantsTwoLines = els.statusbar.scrollHeight > singleLineHeight + 1;
    var nextExtra = wantsTwoLines ? twoLineHeight - singleLineHeight : 0;
    if (Math.abs(nextExtra - statusExtraHeight) < 1) return;
    statusExtraHeight = nextExtra;
    syncTauriWindowSize(getLayoutWindowWidth());
  });
}

function setLoginWatchAbortVisible(visible) {
  els.abortLoginWatchBtn.hidden = !visible;
  els.statusbar.classList.toggle("watching-login", !!visible);
  showLatestTiboPostIfIdle();
  updateStatusHeight();
}

function stopLoginWatch(statusText) {
  loginWatchRunId += 1;
  if (loginWatchTimer) {
    window.clearInterval(loginWatchTimer);
    loginWatchTimer = null;
  }
  loginStableSignature = "";
  loginStableCount = 0;
  setLoginWatchAbortVisible(false);
  if (statusText) setStatus(statusText);
}

function showToast(text) {
  els.toast.textContent = text;
  els.toast.classList.add("show");
  window.setTimeout(function () {
    els.toast.classList.remove("show");
  }, 1700);
}

function formatPlan(value) {
  var text = String(value || "-").trim();
  if (!text || text === "-") return "-";
  return text.charAt(0).toUpperCase() + text.slice(1);
}

function formatPlanWithExpiry(plan, expiresAt) {
  var planText = formatPlan(plan);
  var expiryText = String(expiresAt || "").trim();
  if (!expiryText || expiryText === "-") return planText;
  var dateMatch = expiryText.match(/^\d{4}-\d{2}-\d{2}/);
  return planText + " (Exp. " + (dateMatch ? dateMatch[0] : expiryText) + ")";
}

function formatProfilePlan(profile) {
  if (profile && profile.source === "purchase") {
    if (profile.productId === "monthly_30d") return t("purchasedMonthly");
    if (profile.productId === "weekly_100_3d") return t("purchasedWeekly");
  }
  return formatPlanWithExpiry(profile && profile.plan, profile && profile.expiresAt);
}

function formatEmail(value) {
  var email = String(value || "-").trim();
  if (showEmails || email === "-" || email.indexOf("@") <= 0) return email;
  return email.replace(/^[^@]+/, function (localPart) {
    return "*".repeat(localPart.length);
  });
}

function displayedFiveHourLeft(profile) {
  return profile.secondaryLeft === 0 ? 0 : profile.primaryLeft;
}

function formatUsagePercent(value) {
  return typeof value === "number" ? Math.round(value) + "%" : "-";
}

function getMockProfileCount() {
  var value = new URLSearchParams(window.location.search).get("mock");
  if (value !== "3" && value !== "4") return 0;
  return Number(value);
}

function buildMockProfiles(count) {
  var items = [
    {
      name: "Mira Chen",
      email: "mira.chen@northmail.dev",
      plan: "plus",
      expiresAt: "2026-08-31T00:00:00Z",
      reset: "08-18 16:28 | 08-24 16:28",
      primaryLeft: 69.0,
      secondaryLeft: 91.0,
      active: false,
      authText: "{\n  \"account\": \"Mira Chen\",\n  \"email\": \"mira.chen@northmail.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Alex Morgan",
      email: "alex.morgan@orbitmail.dev",
      plan: "plus",
      expiresAt: "2026-09-12T00:00:00Z",
      reset: "08-18 13:36 | 08-24 13:36",
      primaryLeft: 15.0,
      secondaryLeft: 43.0,
      active: true,
      authText: "{\n  \"account\": \"Alex Morgan\",\n  \"email\": \"alex.morgan@orbitmail.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Noah Rivera",
      email: "noah.rivera@starlane.dev",
      plan: "plus",
      expiresAt: "2026-09-25T00:00:00Z",
      reset: "08-18 14:19 | 08-24 14:19",
      primaryLeft: 74.0,
      secondaryLeft: 82.0,
      active: false,
      authText: "{\n  \"account\": \"Noah Rivera\",\n  \"email\": \"noah.rivera@starlane.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Iris Walker",
      email: "iris.walker@bluepost.dev",
      plan: "team",
      expiresAt: "2026-10-08T00:00:00Z",
      reset: "08-19 09:12 | 08-25 09:12",
      primaryLeft: 86.0,
      secondaryLeft: 96.0,
      active: false,
      authText: "{\n  \"account\": \"Iris Walker\",\n  \"email\": \"iris.walker@bluepost.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    }
  ];
  return items.slice(0, count);
}

function showConfirmDialog(title, message) {
  return new Promise(function (resolve) {
    confirmDialogOpen = true;
    els.confirmTitle.textContent = title;
    els.confirmMessage.textContent = message;
    els.confirmInput.hidden = true;
    els.confirmChoice.hidden = true;
    els.confirmInput.value = "";
    els.confirmLayer.setAttribute("aria-hidden", "false");
    els.confirmLayer.classList.add("show");
    els.confirmOkBtn.focus();

    function cleanup(result) {
      confirmDialogOpen = false;
      els.confirmLayer.classList.remove("show");
      els.confirmLayer.setAttribute("aria-hidden", "true");
      els.confirmOkBtn.onclick = null;
      els.confirmCancelBtn.onclick = null;
      els.confirmLayer.onclick = null;
      els.confirmInput.onkeydown = null;
      resolve(result);
    }

    els.confirmOkBtn.onclick = function () {
      cleanup(true);
    };
    els.confirmCancelBtn.onclick = function () {
      cleanup(false);
    };
    els.confirmLayer.onclick = function (event) {
      if (event.target === els.confirmLayer) cleanup(false);
    };
  });
}

function showPromptDialog(title, message, placeholder, inputType) {
  return new Promise(function (resolve) {
    confirmDialogOpen = true;
    els.confirmTitle.textContent = title;
    els.confirmMessage.textContent = message;
    els.confirmInput.hidden = false;
    els.confirmChoice.hidden = true;
    els.confirmInput.value = "";
    els.confirmInput.placeholder = placeholder || "";
    els.confirmInput.type = inputType === "password" ? "password" : "text";
    els.confirmInput.autocomplete = inputType === "password" ? "current-password" : "off";
    els.confirmLayer.setAttribute("aria-hidden", "false");
    els.confirmLayer.classList.add("show");
    els.confirmInput.focus();

    function cleanup(result) {
      confirmDialogOpen = false;
      els.confirmLayer.classList.remove("show");
      els.confirmLayer.setAttribute("aria-hidden", "true");
      els.confirmInput.hidden = true;
      els.confirmInput.type = "text";
      els.confirmInput.autocomplete = "off";
      els.confirmOkBtn.onclick = null;
      els.confirmCancelBtn.onclick = null;
      els.confirmLayer.onclick = null;
      els.confirmInput.onkeydown = null;
      resolve(result);
    }

    els.confirmOkBtn.onclick = function () {
      cleanup(els.confirmInput.value);
    };
    els.confirmCancelBtn.onclick = function () {
      cleanup(null);
    };
    els.confirmLayer.onclick = function (event) {
      if (event.target === els.confirmLayer) cleanup(null);
    };
    els.confirmInput.onkeydown = function (event) {
      if (event.key === "Enter") cleanup(els.confirmInput.value);
      if (event.key === "Escape") cleanup(null);
    };
  });
}

function showPoolChoiceDialog(profileName) {
  return new Promise(function (resolve) {
    var selectedPool = adminInventoryFilter === "weekly" ? "weekly" : "monthly";
    var buttons = els.confirmChoice.querySelectorAll("[data-confirm-choice]");
    confirmDialogOpen = true;
    els.confirmTitle.textContent = t("chooseUploadPool");
    els.confirmMessage.textContent = t("chooseUploadPoolMessage") + " " + profileName;
    els.confirmInput.hidden = true;
    els.confirmChoice.hidden = false;
    els.confirmLayer.setAttribute("aria-hidden", "false");
    els.confirmLayer.classList.add("show");

    function renderChoice() {
      for (var i = 0; i < buttons.length; i += 1) {
        var value = buttons[i].getAttribute("data-confirm-choice");
        buttons[i].textContent = value === "weekly" ? t("filterWeekly") : t("filterMonthly");
        buttons[i].classList.toggle("active", value === selectedPool);
      }
    }

    function cleanup(result) {
      confirmDialogOpen = false;
      els.confirmLayer.classList.remove("show");
      els.confirmLayer.setAttribute("aria-hidden", "true");
      els.confirmChoice.hidden = true;
      els.confirmChoice.onclick = null;
      els.confirmOkBtn.onclick = null;
      els.confirmCancelBtn.onclick = null;
      els.confirmLayer.onclick = null;
      resolve(result);
    }

    els.confirmChoice.onclick = function (event) {
      var button = event.target.closest("[data-confirm-choice]");
      if (!button) return;
      selectedPool = button.getAttribute("data-confirm-choice") === "weekly" ? "weekly" : "monthly";
      renderChoice();
    };
    els.confirmOkBtn.onclick = function () { cleanup(selectedPool); };
    els.confirmCancelBtn.onclick = function () { cleanup(null); };
    els.confirmLayer.onclick = function (event) {
      if (event.target === els.confirmLayer) cleanup(null);
    };
    renderChoice();
    els.confirmOkBtn.focus();
  });
}

function showPaymentLayer(productId) {
  paymentSession = { payurl: "", productId: productId || selectedStoreProduct };
  els.paymentLayer.classList.add("show");
  els.paymentLayer.setAttribute("aria-hidden", "false");
  els.paymentSubtitle.textContent = "";
  renderPaymentLoading(t("creatingOrder"));
}

function renderPaymentLoading(stageText) {
  els.paymentBody.innerHTML =
    '<div class="payment-loading">' +
    '<div class="payment-spinner"></div>' +
    '<p>' + escapeHtml(stageText) + "</p>" +
    "</div>";
}

function closePaymentLayer(options) {
  var orderId = paymentSession && paymentSession.cloudOrderId
    ? paymentSession.cloudOrderId
    : loadPendingOrderId();
  var shouldAbandon = (!options || options.silent !== true) && !!orderId && cloudbaseState.configured;
  paymentSession = null;
  els.paymentLayer.classList.remove("show");
  els.paymentLayer.setAttribute("aria-hidden", "true");
  renderPaymentLoading(t("creatingOrder"));
  els.paymentSubtitle.textContent = "";
  if (shouldAbandon) {
    savePendingOrderId("");
    stopDeliveryPolling();
    invokeBackend("cloudbase_abandon_order", { orderId: orderId }).then(function () {
      return refreshStoreCatalog();
    }).catch(function (error) {
      console.warn("payment reservation release failed", error);
    });
  }
  if (!options || options.silent !== true) setStatus("Payment cancelled.");
}

function renderPaymentResult(result) {
  if (!paymentSession) return;
  paymentSession = Object.assign({}, paymentSession, result || {});
  var tradeNo = result && result.tradeNo ? result.tradeNo : "";
  var qrImage = result && result.qrImage ? result.qrImage : "";
  var captureDebug = result && result.captureDebug ? result.captureDebug : "";
  els.paymentSubtitle.textContent = "";
  if (qrImage) {
    els.paymentBody.innerHTML =
      '<div class="payment-qr-wrap">' +
      '<img class="payment-qr" src="' + escapeHtml(qrImage) + '" alt="Payment QR code" />' +
      '<p>' + escapeHtml(currentLanguage === "zh-cn" ? "请使用支付 App 扫描二维码。" : "Scan this QR code with your payment app.") + "</p>" +
      "</div>";
    return;
  }
  els.paymentBody.innerHTML =
    '<div class="payment-message">' +
    "<strong>QR code was not found.</strong>" +
    "<p>The order was created, but the cashier capture did not return a QR image.</p>" +
    (captureDebug ? "<p>" + escapeHtml(captureDebug) + "</p>" : "") +
    "</div>";
}

function getStoreProduct(productId) {
  return storeProducts[productId] || storeProducts.monthly;
}

function applyStoreCatalog(result) {
  var products = result && Array.isArray(result.products) ? result.products : [];
  storeCatalogProducts = products;
  storePageHtml = result && typeof result.storePageHtml === "string" ? result.storePageHtml : "";
  storeInventoryLoaded = true;
  Object.keys(storeProducts).forEach(function (key) {
    var localProduct = storeProducts[key];
    var cloudProduct = products.find(function (item) {
      return item.id === localProduct.cloudProductId;
    });
    localProduct.availableCount = cloudProduct ? Number(cloudProduct.availableCount || 0) : 0;
    if (cloudProduct && Number.isFinite(Number(cloudProduct.price))) {
      localProduct.priceNumber = Number(cloudProduct.price);
      localProduct.price = "&yen;" + localProduct.priceNumber;
    }
    if (cloudProduct && cloudProduct.goodsKey) localProduct.goodsKey = String(cloudProduct.goodsKey);
    if (cloudProduct && cloudProduct.itemUrl) localProduct.itemUrl = String(cloudProduct.itemUrl);
  });
}

function updateOpenStoreCatalog() {
  var frame = document.getElementById("storeCloudFrame");
  if (!frame || !frame.contentWindow) return;
  var products = storeCatalogProducts.map(function (product) {
    var local = Object.keys(storeProducts).map(function (key) { return storeProducts[key]; }).find(function (item) {
      return item.cloudProductId === product.id;
    });
    return local ? Object.assign({}, product, { availableCount: local.availableCount, price: local.priceNumber }) : product;
  });
  frame.contentWindow.postMessage({ type: "codex-switch-store-catalog", products: products }, "*");
}

function isStoreProductAvailable(productId) {
  return storeInventoryLoaded && Number(getStoreProduct(productId).availableCount || 0) > 0;
}

function storeAvailabilityLabel(productId) {
  if (!storeInventoryLoaded) return t("checkingStock");
  var count = Number(getStoreProduct(productId).availableCount || 0);
  return count > 0 ? t("inStock") + " " + count : t("soldOut");
}

function refreshStoreCatalog() {
  var statusPromise = cloudbaseState.configured
    ? Promise.resolve(cloudbaseState)
    : invokeBackend("get_cloudbase_status").then(function (status) {
        cloudbaseState = status || cloudbaseState;
        return cloudbaseState;
      });
  return statusPromise.then(function (status) {
    if (!status || !status.configured) {
      applyStoreCatalog({ products: [] });
      if (sideMode === "store" && sideOpen) renderStore();
      return null;
    }
    return invokeBackend("cloudbase_bootstrap");
  }).then(function (result) {
    if (!result) return null;
    if (result && result.identity) cloudbaseIdentity = result.identity;
    applyStoreCatalog(result);
    if (sideMode === "store" && sideOpen) {
      if (document.getElementById("storeCloudFrame")) updateOpenStoreCatalog();
      else renderStore();
    }
    return result;
  });
}

function createClientOrderId() {
  if (window.crypto && typeof window.crypto.randomUUID === "function") {
    return "csw_" + window.crypto.randomUUID().replace(/-/g, "");
  }
  return "csw_" + Date.now().toString(36) + Math.random().toString(36).slice(2, 14);
}

function loadPendingOrderId() {
  try {
    return String(window.localStorage.getItem(pendingOrderStorageKey) || "").trim();
  } catch (error) {
    return "";
  }
}

function savePendingOrderId(orderId) {
  try {
    if (orderId) window.localStorage.setItem(pendingOrderStorageKey, orderId);
    else window.localStorage.removeItem(pendingOrderStorageKey);
  } catch (error) {
    console.warn("pending order persistence failed", error);
  }
}

function stopDeliveryPolling() {
  if (deliveryPollTimer) window.clearInterval(deliveryPollTimer);
  deliveryPollTimer = null;
  deliveryPollStartedAt = 0;
}

function profileOrderIsInstalled(orderId) {
  if (!orderId) return false;
  return profiles.some(function (profile) {
    return profile.orderId === orderId && profile.source === "purchase";
  });
}

function syncPurchasedProfiles(options) {
  var silent = options && options.silent;
  if (!cloudbaseState.configured || deliverySyncRunning) return Promise.resolve(null);
  deliverySyncRunning = true;
  return invokeBackend("cloudbase_sync_deliveries").then(function (result) {
    if (result && result.snapshot) applySnapshot(result.snapshot);
    var delivered = result && Array.isArray(result.deliveredProfiles) ? result.deliveredProfiles : [];
    var errors = result && Array.isArray(result.errors) ? result.errors : [];
    var pendingOrderId = loadPendingOrderId();
    if (delivered.length || profileOrderIsInstalled(pendingOrderId)) {
      savePendingOrderId("");
      stopDeliveryPolling();
      closePaymentLayer({ silent: true });
      setStatus(t("deliveryComplete") + ": " + delivered.join(", "));
      showToast(t("deliveryComplete"));
      delivered.forEach(function (profileName) {
        refreshOneProfileUsage(profileName, { silent: true });
      });
    } else if (errors.length && !silent) {
      setStatus("Delivery failed: " + errors[0]);
    }
    return result;
  }).catch(function (error) {
    if (!silent) setStatus("Delivery sync failed: " + String(error));
    return null;
  }).finally(function () {
    deliverySyncRunning = false;
  });
}

function pollPendingOrder() {
  var orderId = loadPendingOrderId();
  if (!orderId || !cloudbaseState.configured || paymentReconcileRunning) return Promise.resolve(null);
  paymentReconcileRunning = true;
  return invokeBackend("cloudbase_reconcile_payment", { orderId: orderId }).catch(function (error) {
    console.warn("provider payment reconciliation failed", error);
    return invokeBackend("cloudbase_get_order", { orderId: orderId });
  }).then(function (result) {
    var order = result && result.order ? result.order : null;
    if (!order) return null;
    if (order.paymentStatus === "paid" && order.fulfillmentStatus === "allocated") {
      if (paymentSession) renderPaymentLoading(t("paymentConfirmed"));
      setStatus(t("paymentConfirmed"));
      return syncPurchasedProfiles();
    }
    if (["expired", "failed", "refunded", "cancelled"].indexOf(order.paymentStatus) >= 0) {
      savePendingOrderId("");
      stopDeliveryPolling();
    }
    return order;
  }).catch(function (error) {
    console.warn("pending order poll failed", error);
    return null;
  }).finally(function () {
    paymentReconcileRunning = false;
  });
}

function startDeliveryPolling(orderId) {
  if (orderId) savePendingOrderId(orderId);
  if (!loadPendingOrderId()) return;
  stopDeliveryPolling();
  deliveryPollStartedAt = Date.now();
  pollPendingOrder();
  deliveryPollTimer = window.setInterval(function () {
    if (Date.now() - deliveryPollStartedAt > 15 * 60 * 1000) {
      stopDeliveryPolling();
      return;
    }
    pollPendingOrder();
  }, 4000);
}

function startStorePayment(productId) {
  var product = getStoreProduct(productId || selectedStoreProduct);
  if (!isStoreProductAvailable(product.id)) {
    showToast(currentLanguage === "zh-cn" ? "当前套餐暂无可用库存" : "This product is out of stock");
    refreshStoreCatalog().catch(function (error) {
      console.warn("store inventory refresh failed", error);
    });
    return;
  }
  selectedStoreProduct = product.id;
  if (sideMode === "store" && sideOpen) renderStore();
  showPaymentLayer(product.id);
  setStatus(currentLanguage === "zh-cn" ? "正在建立支付订单..." : "Creating payment order...");
  var cloudDraftPromise = cloudbaseState.configured
    ? invokeBackend("cloudbase_create_order_draft", {
        productId: product.cloudProductId,
        clientOrderId: createClientOrderId()
      })
    : Promise.resolve(null);

  cloudDraftPromise.then(function (cloudDraft) {
    var cloudOrder = cloudDraft && cloudDraft.order ? cloudDraft.order : null;
    if (!cloudOrder || !cloudOrder._id || !cloudOrder.productSnapshot) {
      throw new Error(currentLanguage === "zh-cn" ? "云端订单创建失败，已停止支付。" : "Cloud order creation failed. Payment was stopped.");
    }
    if (!paymentSession) {
      if (cloudOrder && cloudOrder._id) {
        invokeBackend("cloudbase_abandon_order", { orderId: cloudOrder._id }).catch(function () {});
      }
      return null;
    }
    if (cloudOrder && cloudOrder.reservedProfileId) {
      product.availableCount = Math.max(0, Number(product.availableCount || 0) - 1);
      if (sideMode === "store" && sideOpen) renderStore();
    }
    paymentSession.cloudOrderId = cloudOrder && cloudOrder._id ? cloudOrder._id : "";
    paymentSession.expectedAmount = Number(cloudOrder.productSnapshot.price);
    if (paymentSession.cloudOrderId) savePendingOrderId(paymentSession.cloudOrderId);
    if (paymentSession.expectedAmount !== product.priceNumber) {
      throw new Error(currentLanguage === "zh-cn"
        ? "云端订单金额与套餐价格不一致，已停止支付。"
        : "Cloud order amount does not match the product price. Payment was stopped.");
    }
    return invokeBackend("create_store_payment_order", {
      goodsKey: product.goodsKey,
      itemUrl: product.itemUrl
    });
  }).then(function (order) {
    if (!paymentSession) return null;
    var cashierAmount = Number(order && order.totalAmount);
    if (!order || !order.totalAmount || !Number.isFinite(cashierAmount) ||
        Math.abs(cashierAmount - paymentSession.expectedAmount) > 0.001) {
      throw new Error(currentLanguage === "zh-cn"
        ? "收银台金额与套餐价格不一致，已停止支付。请稍后重试。"
        : "Cashier amount does not match the product price. Payment was stopped.");
    }
    var cloudOrderId = paymentSession.cloudOrderId || "";
    paymentSession = Object.assign({
      productId: product.id,
      itemUrl: product.itemUrl,
      cloudOrderId: cloudOrderId
    }, order || {});
    var bindPromise = cloudOrderId
      ? invokeBackend("cloudbase_attach_payment_reference", {
          orderId: cloudOrderId,
          tradeNo: order && order.tradeNo ? order.tradeNo : ""
        })
      : Promise.resolve(null);
    return bindPromise.then(function () {
      if (cloudOrderId) startDeliveryPolling(cloudOrderId);
      renderPaymentLoading(t("fetchingQr"));
      setStatus(currentLanguage === "zh-cn" ? "正在获取支付二维码..." : "Fetching payment QR code...");
      return invokeBackend("capture_store_payment_qr", {
        tradeNo: order && order.tradeNo ? order.tradeNo : "",
        totalAmount: order && order.totalAmount ? order.totalAmount : null,
        payurl: order && order.payurl ? order.payurl : ""
      });
    });
  }).then(function (result) {
    if (!result || !paymentSession) return;
    setStatus("Payment QR code ready.");
    renderPaymentResult(result);
  }).catch(function (error) {
    if (!paymentSession) return;
    if (paymentSession.cloudOrderId && !paymentSession.tradeNo) {
      invokeBackend("cloudbase_abandon_order", { orderId: paymentSession.cloudOrderId }).catch(function () {});
      savePendingOrderId("");
    }
    var message = error && error.message ? error.message : String(error);
    els.paymentSubtitle.textContent = "Payment failed";
    els.paymentBody.innerHTML =
      '<div class="payment-message">' +
      "<strong>Could not create payment.</strong>" +
      "<p>" + escapeHtml(message) + "</p>" +
      "</div>";
    paymentSession = {};
  });
}

function applyProfileLayoutState() {
  var canToggleColumns = profiles.length >= 4;
  if (!canToggleColumns || adminUnlocked) forceSingleColumn = false;
  layoutMode = adminUnlocked || (canToggleColumns && !forceSingleColumn) ? "double" : "single";
  els.appShell.classList.toggle("double-mode", layoutMode === "double");
  els.appShell.classList.toggle("single-mode", layoutMode === "single");
  els.appShell.classList.toggle("force-single-column", forceSingleColumn);
  syncAdminWorkspaceState(false);
  els.columnToggleBtn.hidden = adminUnlocked || !canToggleColumns;
  els.columnToggleBtn.classList.toggle("is-forced", forceSingleColumn);
  els.columnToggleBtn.title = forceSingleColumn ? t("restoreTwoColumns") : t("singleColumn");
  els.columnToggleBtn.setAttribute("aria-label", forceSingleColumn ? t("restoreTwoColumns") : t("singleColumn"));
  syncTauriWindowSize(getLayoutWindowWidth());
  els.profileList.classList.toggle("single-column", layoutMode === "single");
  updateProfileHintVisibility();
}

function renderProfiles() {
  els.profileList.innerHTML = "";
  applyProfileLayoutState();
  if (profiles.length === 0) {
    var emptyCard = document.createElement("button");
    emptyCard.className = "empty-profile-card";
    emptyCard.innerHTML = '<span class="empty-add-circle" aria-hidden="true"></span>';
    emptyCard.onclick = loginNewAccount;
    els.profileList.appendChild(emptyCard);
    updateActionButtons();
    return;
  }

  for (var i = 0; i < profiles.length; i += 1) {
    var profile = profiles[i];
    var card = document.createElement("article");
    card.className = "profile-card";
    if (profile.active) card.className += " active";
    if (profile.name === selectedVisualName) card.className += " selected";
    card.setAttribute("data-profile", profile.name);
    card.setAttribute("data-index", String(i));

    var fiveHourLeft = displayedFiveHourLeft(profile);
    var hasUsage = typeof fiveHourLeft === "number";
    var warn = hasUsage && fiveHourLeft < 35;
    var mid = hasUsage && fiveHourLeft >= 35 && fiveHourLeft < 70;
    var usageTone = warn ? "warn" : (mid ? "mid" : "");
    var usageText = formatUsagePercent(fiveHourLeft);
    var usageWidth = hasUsage ? fiveHourLeft : 0;
    var hasWeeklyUsage = typeof profile.secondaryLeft === "number";
    var weeklyUsageText = formatUsagePercent(profile.secondaryLeft);
    var weeklyUsageWidth = hasWeeklyUsage ? profile.secondaryLeft : 0;
    var resetParts = String(profile.reset || "-").split(" | ");
    var fiveHourReset = resetParts[0] || "-";
    var weeklyReset = resetParts.length > 1 ? resetParts.slice(1).join(" | ") : "";
    var canStartFiveHour = !profile.active && !fiveHourRequestRunning && !isMockMode;
    card.innerHTML =
      '<div class="profile-card-header">' +
      '  <div class="profile-name">' +
      escapeHtml(profile.name) +
      "  </div>" +
      (profile.active ? '<span class="profile-badge">active</span>' : "") +
      '  <button class="profile-config-button" data-profile-config="' +
      escapeHtml(profile.name) +
      '" title="' + escapeHtml(t("configContent")) + '" aria-label="' + escapeHtml(t("configContent")) + '">' +
      '    <img src="./assets/config.png" alt="" />' +
      "  </button>" +
      "</div>" +
      '<div class="profile-lines">' +
      "  <div><span>" + escapeHtml(t("plan")) + "</span><b>" +
      escapeHtml(formatProfilePlan(profile)) +
      "</b></div>" +
      "  <div><span>" + escapeHtml(t("email")) + "</span><b>" +
      escapeHtml(formatEmail(profile.email)) +
      "</b></div>" +
      "  <div><span>" + escapeHtml(t("reset")) + "</span><b>" +
      '<span class="five-hour-reset-time" data-five-hour-profile="' + escapeHtml(profile.name) + '"' +
      (canStartFiveHour ? ' role="button" tabindex="0" aria-label="' +
        escapeHtml(t("startFiveHour") + ": " + profile.name) + '"' : "") +
      '>' + (fiveHourConfirmedProfiles.has(profile.name) ? '<i class="five-hour-success-dot" role="img" aria-label="' + escapeHtml(t("startFiveHourConfirmed")) + '" title="' + escapeHtml(t("startFiveHourConfirmed")) + '"></i>' : '') + escapeHtml(fiveHourReset) + '</span>' +
      (weeklyReset ? ' | ' + escapeHtml(weeklyReset) : "") +
      "</b></div>" +
      "</div>" +
      '<div class="usage-strip">' +
      '  <div class="usage-row"><span class="usage-row-label">' + escapeHtml(t("usageLeftCombined")) + '</span>' +
      '    <span class="usage-values"><strong class="usage-value-primary ' + usageTone + '">' + usageText + '</strong>' +
      '      <span class="usage-value-separator">|</span><strong class="usage-value-weekly">' + weeklyUsageText + "</strong></span>" +
      "  </div>" +
      '  <div class="usage-bar">' +
      (hasWeeklyUsage
        ? '    <div class="usage-layer usage-layer-weekly"><div class="usage-fill usage-fill-weekly" style="width:' + weeklyUsageWidth + '%"></div></div>'
        : "") +
      '    <div class="usage-layer usage-layer-primary"><div class="usage-fill ' + usageTone + '" style="width:' + usageWidth + '%"></div></div>' +
      "  </div>" +
      "</div>" +
      "";

    card.onclick = function (event) {
      if (suppressNextCardClick) {
        suppressNextCardClick = false;
        return;
      }
      if (event.target.closest(".profile-config-button, .five-hour-reset-time[role='button']")) return;
      selectedName = this.getAttribute("data-profile");
      fiveHourConfirmedProfiles.delete(selectedName);
      selectedVisualName = selectedName;
      scheduleSelectionClear();
      renderProfiles();
      if (sideMode === "config") renderConfig();
      setStatus("Selected profile: " + selectedName);
    };
    card.querySelector(".profile-config-button").onclick = function (event) {
      event.stopPropagation();
      renderProfileConfig(this.getAttribute("data-profile-config"));
    };
    card.querySelector(".profile-config-button").onpointerdown = function (event) {
      event.stopPropagation();
    };
    var resetTime = card.querySelector(".five-hour-reset-time[role='button']");
    if (resetTime) {
      resetTime.onclick = function (event) {
        event.stopPropagation();
        startFiveHourWindow(this.getAttribute("data-five-hour-profile"));
      };
      resetTime.onkeydown = function (event) {
        if (event.key !== "Enter" && event.key !== " ") return;
        event.preventDefault();
        this.click();
      };
      resetTime.onpointerdown = function (event) {
        event.stopPropagation();
      };
    }
    card.onpointerdown = beginProfileCardPointer;

    els.profileList.appendChild(card);
  }
  updateActionButtons();
}

function scheduleSelectionClear() {
  if (selectedClearTimer) {
    window.clearTimeout(selectedClearTimer);
  }
  selectedClearTimer = window.setTimeout(function () {
    selectedVisualName = "";
    selectedClearTimer = null;
    renderProfiles();
  }, 3000);
}

function canDragReorderProfiles() {
  return layoutMode === "single" && profiles.length > 1 && !sideOpen;
}

function beginProfileCardPointer(event) {
  if (event.button !== 0 || !canDragReorderProfiles()) return;
  if (dragState) return;
  var card = event.currentTarget;
  var startY = event.clientY;
  var startX = event.clientX;
  var pointerId = event.pointerId;
  var index = Number(card.getAttribute("data-index"));

  function cleanupPendingDrag() {
    window.removeEventListener("pointermove", moveBeforeDrag);
    window.removeEventListener("pointerup", cleanupPendingDrag);
    window.removeEventListener("pointercancel", cleanupPendingDrag);
  }

  function moveBeforeDrag(moveEvent) {
    if (moveEvent.pointerId !== pointerId) return;
    if (Math.abs(moveEvent.clientY - startY) > 4 || Math.abs(moveEvent.clientX - startX) > 4) {
      cleanupPendingDrag();
      startProfileCardDrag(card, pointerId, index, startY);
      moveProfileCardDrag(moveEvent);
    }
  }

  window.addEventListener("pointermove", moveBeforeDrag);
  window.addEventListener("pointerup", cleanupPendingDrag, { once: true });
  window.addEventListener("pointercancel", cleanupPendingDrag, { once: true });
}

function startProfileCardDrag(card, pointerId, originalIndex, startY) {
  if (dragState || !canDragReorderProfiles()) return;
  var rect = card.getBoundingClientRect();
  var listRect = els.profileList.getBoundingClientRect();
  var placeholder = document.createElement("div");
  placeholder.className = "profile-drag-placeholder";
  placeholder.style.height = rect.height + "px";
  placeholder.style.margin = "0";
  card.parentNode.insertBefore(placeholder, card.nextSibling);

  dragState = {
    card: card,
    placeholder: placeholder,
    originalIndex: originalIndex,
    pointerId: pointerId,
    offsetY: startY - rect.top,
    listTop: listRect.top,
    left: rect.left,
    top: rect.top,
    width: rect.width,
    height: rect.height,
    placeholderBefore: null
  };

  suppressNextCardClick = true;
  els.profileList.classList.add("reordering");
  card.classList.add("dragging");
  card.style.left = rect.left + "px";
  card.style.top = rect.top + "px";
  card.style.width = rect.width + "px";
  card.style.height = rect.height + "px";
  card.style.transform = "translate3d(0, 0, 0)";

  window.addEventListener("pointermove", moveProfileCardDrag);
  window.addEventListener("pointerup", finishProfileCardDrag);
  window.addEventListener("pointercancel", cancelProfileCardDrag);
}

function moveProfileCardDrag(event) {
  if (!dragState || event.pointerId !== dragState.pointerId) return;
  event.preventDefault();
  var y = event.clientY - dragState.offsetY;
  dragState.card.style.transform = "translate3d(0, " + (y - dragState.top) + "px, 0)";

  var midpoint = event.clientY;
  var cards = Array.from(els.profileList.querySelectorAll(".profile-card:not(.dragging)"));
  var beforeNode = null;
  for (var i = 0; i < cards.length; i += 1) {
    var rect = cards[i].getBoundingClientRect();
    if (midpoint < rect.top + rect.height / 2) {
      beforeNode = cards[i];
      break;
    }
  }
  if (dragState.placeholderBefore === beforeNode) return;
  dragState.placeholderBefore = beforeNode;
  moveProfilePlaceholder(beforeNode);
}

function moveProfilePlaceholder(beforeNode) {
  if (!dragState) return;
  var cards = Array.from(els.profileList.querySelectorAll(".profile-card:not(.dragging)"));
  var oldRects = new Map();
  for (var i = 0; i < cards.length; i += 1) {
    oldRects.set(cards[i], cards[i].getBoundingClientRect());
  }

  if (beforeNode) {
    els.profileList.insertBefore(dragState.placeholder, beforeNode);
  } else {
    els.profileList.appendChild(dragState.placeholder);
  }

  for (var j = 0; j < cards.length; j += 1) {
    var oldRect = oldRects.get(cards[j]);
    var newRect = cards[j].getBoundingClientRect();
    var deltaY = oldRect.top - newRect.top;
    if (Math.abs(deltaY) < 1) continue;
    cards[j].style.transition = "none";
    cards[j].style.transform = "translate3d(0, " + deltaY + "px, 0)";
    cards[j].getBoundingClientRect();
    cards[j].style.transition = "transform 150ms cubic-bezier(0.22, 1, 0.36, 1)";
    cards[j].style.transform = "translate3d(0, 0, 0)";
  }
}

function finishProfileCardDrag(event) {
  if (!dragState || event.pointerId !== dragState.pointerId) return;
  var state = dragState;
  var placeholderIndex = Array.from(els.profileList.children)
    .filter(function (node) {
      return node === state.placeholder || node.classList.contains("profile-card");
    })
    .indexOf(state.placeholder);
  var moved = profiles.splice(state.originalIndex, 1)[0];
  if (placeholderIndex > state.originalIndex) placeholderIndex -= 1;
  profiles.splice(Math.max(0, placeholderIndex), 0, moved);
  saveProfileOrder();
  cleanupProfileCardDrag();
  renderProfiles();
  setStatus("Profile order updated.");
}

function cancelProfileCardDrag(event) {
  if (!dragState || event.pointerId !== dragState.pointerId) return;
  cleanupProfileCardDrag();
  renderProfiles();
}

function cleanupProfileCardDrag() {
  if (!dragState) return;
  var card = dragState.card;
  window.removeEventListener("pointermove", moveProfileCardDrag);
  window.removeEventListener("pointerup", finishProfileCardDrag);
  window.removeEventListener("pointercancel", cancelProfileCardDrag);
  card.classList.remove("dragging");
  card.style.left = "";
  card.style.top = "";
  card.style.width = "";
  card.style.height = "";
  card.style.transform = "";
  if (dragState.placeholder.parentNode) {
    dragState.placeholder.parentNode.removeChild(dragState.placeholder);
  }
  els.profileList.classList.remove("reordering");
  dragState = null;
}

function closeSide() {
  sideMode = "";
  sideOpen = false;
  if (storePanelResizeObserver) {
    storePanelResizeObserver.disconnect();
    storePanelResizeObserver = null;
  }
  els.sideBody.style.width = "";
  var invoke = getTauriInvoke();
  els.appShell.classList.remove("panel-open");
  els.appShell.classList.remove("panel-config", "panel-more", "panel-store");
  syncAdminWorkspaceState(false);
  updateProfileHintVisibility();
  els.sidePanel.setAttribute("aria-hidden", "true");
  els.sideControls.innerHTML = "";
  syncTauriWindowSize(getLayoutWindowWidth());
  if (!invoke) return;
  invoke("hide_side_panel", {
    baseWidth: getLayoutWindowWidth()
  }).catch(function (error) {
      console.warn("hide_side_panel failed", error);
  });
}

function runAfterSideClosed(actionFn) {
  var shouldWait = sideOpen;
  closeSide();
  window.setTimeout(actionFn, shouldWait ? 220 : 0);
}

function getLayoutWindowWidth() {
  if (isAdminWorkspaceOpen()) return adminWindowWidth;
  return layoutMode === "double" ? doubleWindowWidth : singleWindowWidth;
}

function isAdminWorkspaceOpen() {
  return adminUnlocked && layoutMode === "double" && sideOpen && sideMode === "more";
}

function syncAdminWorkspaceState(shouldRender) {
  var visible = isAdminWorkspaceOpen();
  els.appShell.classList.toggle("admin-workspace", visible);
  els.adminInventoryPanel.setAttribute("aria-hidden", visible ? "false" : "true");
  if (shouldRender !== false) renderAdminInventoryPanel();
}

function syncTauriWindowSize(width, done) {
  var invoke = getTauriInvoke();
  var targetWidth = typeof width === "number" ? width : getLayoutWindowWidth();
  if (!invoke) {
    if (done) done();
    return;
  }
  invoke("resize_window", {
    width: targetWidth,
    height: baseWindowHeight + statusExtraHeight
  }).then(function () {
    if (done) done();
  }).catch(function (error) {
    console.warn("resize_window failed", error);
    if (done) done();
  });
}

function getFloatingBarInfo() {
  var active = null;
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].active) {
      active = profiles[i];
      break;
    }
  }
  if (!active) active = getSelectedProfile();
  return {
    title: active && active.name ? active.name : "Codex Switch",
    usage: active ? formatUsagePercent(displayedFiveHourLeft(active)) : "-",
    width: 160
  };
}

function syncFloatingBar(announce) {
  var invoke = getTauriInvoke();
  if (!invoke || isMockMode) return;
  var info = getFloatingBarInfo();
  invoke("set_floating_bar", {
    enabled: floatingBarEnabled,
    title: info.title,
    usage: info.usage,
    width: info.width
  }).then(function () {
    if (announce && floatingBarEnabled) {
      setStatus("Floating bar enabled.");
    }
  }).catch(function (error) {
    console.warn("set_floating_bar failed", error);
    if (announce && floatingBarEnabled) {
      setStatus("Floating bar failed: " + String(error));
    }
  });
}

function setFloatingBarEnabled(enabled) {
  floatingBarEnabled = !!enabled;
  saveFloatingBarEnabled(floatingBarEnabled);
  syncFloatingBar(true);
}

function getTauriInvoke() {
  if (!window.__TAURI__ || !window.__TAURI__.core || !window.__TAURI__.core.invoke) {
    return null;
  }
  return window.__TAURI__.core.invoke;
}

function invokeBackend(commandName, args) {
  var invoke = getTauriInvoke();
  if (!invoke) {
    return Promise.reject(new Error("Tauri backend is not available."));
  }
  return invoke(commandName, args || {});
}

function initializeCloudBase() {
  if (isMockMode) return Promise.resolve();
  return invokeBackend("get_cloudbase_status").then(function (status) {
    cloudbaseState = status || cloudbaseState;
    if (!status || !status.configured) return null;
    return invokeBackend("cloudbase_bootstrap");
  }).then(function (result) {
    if (result && result.identity) cloudbaseIdentity = result.identity;
    applyStoreCatalog(result);
    return result;
  }).catch(function (error) {
    console.warn("CloudBase bootstrap skipped", error);
    return null;
  });
}

function applySnapshot(snapshot, statusText) {
  profiles = snapshot && Array.isArray(snapshot.profiles) ? snapshot.profiles : [];
  applySavedProfileOrder();
  for (var p = 0; p < profiles.length; p += 1) {
    var cachedUsage = usageCache[profiles[p].name];
    if (!cachedUsage) continue;
    profiles[p].plan = cachedUsage.plan || profiles[p].plan;
    profiles[p].primaryLeft = typeof cachedUsage.primaryLeft === "number" ? cachedUsage.primaryLeft : profiles[p].primaryLeft;
    profiles[p].secondaryLeft = typeof cachedUsage.secondaryLeft === "number" ? cachedUsage.secondaryLeft : profiles[p].secondaryLeft;
    profiles[p].expiresAt = profiles[p].expiresAt || cachedUsage.expiresAt || "";
    profiles[p].reset = cachedUsage.reset || profiles[p].reset;
  }
  var hasSelected = false;
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name === selectedName) hasSelected = true;
  }
  if (!hasSelected) {
    selectedName = "";
    for (var j = 0; j < profiles.length; j += 1) {
      if (profiles[j].active) {
        selectedName = profiles[j].name;
        break;
      }
    }
    if (!selectedName && profiles[0]) selectedName = profiles[0].name;
  }
  renderProfiles();
  saveProfileSnapshot();
  if (!isMockMode) updateNativeTray();
  syncFloatingBar();
  if (sideMode === "config" && sideOpen) renderConfig();
  if (statusText) setStatus(statusText);
}

function runBackendAction(commandName, args, busyText, doneText) {
  setStatus(busyText);
  return invokeBackend(commandName, args)
    .then(function (result) {
      if (result && Array.isArray(result.profiles)) {
        applySnapshot(result, doneText || "Done.");
      } else if (result && result.message) {
        setStatus(result.message);
      } else {
        setStatus(doneText || "Done.");
      }
      return result;
    })
    .catch(function (error) {
      var message = error && error.message ? error.message : String(error);
      setStatus("Failed: " + message);
      showToast(("Failed: " + message).slice(0, 96));
      throw error;
    });
}

function loadProfiles() {
  if (isMockMode) {
    applySnapshot({ profiles: buildMockProfiles(mockProfileCount) }, "Mock screenshot mode.");
    return Promise.resolve();
  }
  return runBackendAction("list_profiles", {}, "Loading profiles...", "Profiles loaded.");
}

function applyProfileUsage(usage, options) {
  if (!usage || !usage.profileName) return;
  var shouldRender = !options || options.render !== false;
  var previousUsage = usageCache[usage.profileName] || {};
  usageCache[usage.profileName] = {
    plan: usage.plan,
    primaryLeft: usage.primaryLeft,
    primaryResetAt: usage.primaryResetAt,
    secondaryLeft: usage.secondaryLeft,
    expiresAt: usage.expiresAt || previousUsage.expiresAt || "",
    reset: usage.reset
  };
  saveUsageCache();
  if (autoFiveHourScheduler && !(options && options.fiveHourResult)) {
    var observedProfile = getProfileByName(usage.profileName);
    if (observedProfile) autoFiveHourScheduler.observe(observedProfile, usage);
  }
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name !== usage.profileName) continue;
    profiles[i].plan = usage.plan || profiles[i].plan;
    profiles[i].primaryLeft = typeof usage.primaryLeft === "number" ? usage.primaryLeft : profiles[i].primaryLeft;
    profiles[i].secondaryLeft = typeof usage.secondaryLeft === "number" ? usage.secondaryLeft : profiles[i].secondaryLeft;
    profiles[i].expiresAt = usage.expiresAt || profiles[i].expiresAt || "";
    profiles[i].reset = usage.reset || profiles[i].reset;
    break;
  }
  if (!shouldRender) return;
  renderProfiles();
  saveProfileSnapshot();
  syncFloatingBar();
  if (sideMode === "config" && sideOpen) renderConfig();
}

function clearProfileUsage(profileName) {
  if (!profileName) return;
  delete usageCache[profileName];
  saveUsageCache();
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name !== profileName) continue;
    profiles[i].primaryLeft = null;
    profiles[i].secondaryLeft = null;
    profiles[i].reset = "-";
    break;
  }
  renderProfiles();
  saveProfileSnapshot();
  syncFloatingBar();
}

function showFiveHourProgress(profileName, requestId) {
  var zh = currentLanguage === "zh-cn";
  if (fiveHourStatusRestoreTimer) window.clearTimeout(fiveHourStatusRestoreTimer);
  if (statusIdleTimer) window.clearTimeout(statusIdleTimer);
  fiveHourStatusRestoreTimer = null;
  function display(text) {
    var message = profileName + " · " + text;
    if (fiveHourStatusText === message) return;
    fiveHourStatusText = message;
    renderStatusTicker("", message);
    els.statusText.title = message;
  }
  display(zh ? "正在准备固定 5h 刷新时间…" : "Preparing to fix the 5h reset time…");
  var finished = false;
  var polling = false;
  var labels = {
    refresh_lock_wait: ["等待额度查询完成", "Waiting for the usage query"],
    usage_before_start: ["查询当前额度", "Reading current usage"],
    existing_reset_stability_check: ["核实刷新时间是否已固定", "Checking reset time stability"],
    cli_spawn_start: ["启动独立 CLI 任务", "Starting an isolated CLI task"],
    cli_spawned: ["正在连接并运行简单任务", "Connecting and running the short task"],
    cli_exit: ["CLI 已结束，正在核验结果", "CLI finished; checking the result"],
    chat_cleanup_start: ["清理测试聊天", "Cleaning up the test chat"],
    usage_after_start: ["回读服务端刷新时间", "Reading the server reset time"],
    confirmation_decision: ["核实刷新时间是否稳定", "Checking reset time stability"]
  };
  var timer = window.setInterval(function () {
    if (finished || polling) return;
    polling = true;
    invokeBackend("get_five_hour_progress", { requestId: requestId }).then(function (stage) {
      if (finished) return;
      var label = labels[stage];
      if (label) display(label[zh ? 0 : 1]);
    }).catch(function (error) { console.warn("five-hour progress failed", error); }).finally(function () { polling = false; });
  }, 1000);
  return function (result, error) {
    finished = true;
    window.clearInterval(timer);
    var message;
    if (error) {
      var network = /timed?\s*out|timeout|network/i.test(String(error));
      message = network ? (zh ? "固定失败：网络连接超时，请检查代理后重试。" : "Failed: connection timed out. Check the proxy and retry.") : (zh ? "固定失败：" : "Failed: ") + String(error);
      message += " · " + requestId;
    } else if (result && result.confirmed) {
      fiveHourConfirmedProfiles.add(profileName);
      var resetAt = result.usage && result.usage.primaryResetAt;
      message = zh ? "已确认 5h 刷新时间固定" : "5h reset time confirmed";
      if (resetAt) message += "：" + new Date(resetAt * 1000).toLocaleString(zh ? "zh-CN" : "en-US");
    } else {
      message = (zh ? "尚未确认固定，请稍后重试" : "Reset time not confirmed; try again later") + " · " + requestId;
    }
    display(message);
    var duration = Math.max(5000, Math.min(20000, getTiboTickerDuration(fiveHourStatusText) * 1000));
    fiveHourStatusRestoreTimer = window.setTimeout(function () {
      fiveHourStatusRestoreTimer = null;
      fiveHourStatusText = null;
      els.statusText.removeAttribute("title");
      statusLastOperationAt = 0;
      if (showTiboPost) showLatestTiboPost();
      else renderOperationStatus(t("ready"));
    }, duration);
  };
}

function runFiveHourRequest(profileName, requestId, automatic) {
  if (fiveHourRequestRunning) return Promise.reject(new Error("A five-hour request is already running."));
  var profile = getProfileByName(profileName);
  if (!profile || (profile.active && !automatic)) return Promise.reject(new Error("The selected profile is no longer available for this request."));
  function logStage(stage) {
    invokeBackend("log_five_hour_event", { requestId: requestId, profileName: profileName, stage: stage })
      .catch(function (error) { console.warn("five-hour diagnostic log failed", requestId, error); });
  }
  if (!automatic && autoFiveHourScheduler) autoFiveHourScheduler.markStarted(profile);
  var finishProgress = showFiveHourProgress(profileName, requestId);
  fiveHourRequestRunning = true;
  renderProfiles();
  logStage(automatic ? "automatic_started" : "invoke_sent");
  return invokeBackend("start_five_hour_window", { profileName: profileName, requestId: requestId })
    .then(function (result) {
      finishProgress(result, null);
      logStage(result && result.confirmed ? "ui_confirmed" : "ui_unconfirmed");
      if (result && result.usage) applyProfileUsage(result.usage, { fiveHourResult: true });
      if (!automatic && autoFiveHourScheduler) autoFiveHourScheduler.recordResult(profile, result, null);
      return result;
    })
    .catch(function (error) {
      finishProgress(null, error);
      logStage("ui_failed");
      if (!automatic && autoFiveHourScheduler) autoFiveHourScheduler.recordResult(profile, null, error);
      throw error;
    })
    .finally(function () {
      fiveHourRequestRunning = false;
      renderProfiles();
    });
}

function startFiveHourWindow(profileName) {
  var requestId = crypto.randomUUID();
  function logStage(stage) {
    invokeBackend("log_five_hour_event", { requestId: requestId, profileName: profileName, stage: stage })
      .catch(function (error) { console.warn("five-hour diagnostic log failed", requestId, error); });
  }
  logStage("button_clicked");
  var profile = getProfileByName(profileName);
  if (!profile || profile.active || fiveHourRequestRunning || manualFiveHourConfirmationPending || isMockMode) { logStage("blocked"); return; }
  manualFiveHourConfirmationPending = true;
  return showConfirmDialog(t("startFiveHour"), t("startFiveHourConfirm").replace("{profile}", profileName))
    .then(function (confirmed) {
      manualFiveHourConfirmationPending = false;
      if (!confirmed) { logStage("confirmation_cancelled"); return; }
      if (fiveHourRequestRunning) { logStage("blocked"); return; }
      logStage("confirmation_accepted");
      return runFiveHourRequest(profileName, requestId, false).catch(function () {});
    }).finally(function () { manualFiveHourConfirmationPending = false; });
}

function initializeAutoFiveHour() {
  if (isMockMode || !getTauriInvoke()) return;
  var sessionId = crypto.randomUUID();
  autoFiveHourScheduler = createAutoFiveHourScheduler({
    now: function () { return Math.floor(Date.now() / 1000); },
    load: function () {
      try { return JSON.parse(window.localStorage.getItem(autoFiveHourStateKey) || "{}"); }
      catch (error) { console.warn("load auto five-hour state failed", error); return {}; }
    },
    save: function (state) {
      try { window.localStorage.setItem(autoFiveHourStateKey, JSON.stringify(state)); }
      catch (error) { console.warn("save auto five-hour state failed", error); }
    },
    profiles: function () { return profiles; },
    canRun: function () { return !profileSwitchRunning && !fiveHourRequestRunning && !usageRefreshRunning && !manualFiveHourConfirmationPending && !confirmDialogOpen && !loginWatchTimer; },
    run: function (profile) { return runFiveHourRequest(profile.name, crypto.randomUUID(), true); },
    log: function (stage, profile, details) {
      invokeBackend("log_five_hour_event", { requestId: sessionId, profileName: profile.name, stage: stage, details: details })
        .catch(function (error) { console.warn("auto five-hour diagnostic failed", error); });
    }
  });
  autoFiveHourScheduler.setEnabled(autoFiveHourEnabled);
  // This timer only drains previously observed candidates. It never queries usage or starts an unchecked run.
  autoFiveHourDrainTimer = window.setInterval(function () { autoFiveHourScheduler.drain(); }, 5000);
  window.addEventListener("focus", function () {
    if (autoFiveHourEnabled) refreshAllProfileUsage(true, { silent: true });
  });
}

function setAutoFiveHourEnabled(enabled) {
  autoFiveHourEnabled = !!enabled;
  saveBooleanSetting(autoFiveHourStorageKey, autoFiveHourEnabled);
  if (autoFiveHourScheduler) autoFiveHourScheduler.setEnabled(autoFiveHourEnabled);
  if (autoFiveHourEnabled) refreshAllProfileUsage(true, { silent: true });
}
function refreshOneProfileUsage(profileName, options) {
  if (!profileName) return Promise.resolve();
  var silent = options && options.silent;
  if (!silent) setStatus("Refreshing usage: " + profileName + "...");
  return invokeBackend("refresh_profile_usage", { profileName: profileName })
    .then(function (usage) {
      applyProfileUsage(usage);
      if (!silent) setStatus("Usage refreshed: " + profileName);
      return usage;
    })
    .catch(function (error) {
      console.warn("refresh_profile_usage failed", profileName, error);
      if (!silent) setStatus("Usage refresh skipped: " + profileName);
    });
}

function refreshAllProfileUsage(force, options) {
  if (isMockMode) return Promise.resolve({ refreshed: 0, skipped: 0 });
  if (!force && initialUsageRefreshed) return Promise.resolve({ refreshed: 0, skipped: 0 });
  if (profiles.length === 0) return Promise.resolve({ refreshed: 0, skipped: 0 });
  if (usageRefreshRunning) return Promise.resolve({ refreshed: 0, skipped: 0 });
  var silent = options && options.silent;
  usageRefreshRunning = true;
  initialUsageRefreshed = true;
  var runId = usageRefreshRunId + 1;
  usageRefreshRunId = runId;
  if (!silent) setStatus("Refreshing usage for all profiles...");
  var queue = profiles.slice().sort(function (left, right) {
    return Number(!!right.active) - Number(!!left.active);
  }).map(function (profile) {
    return profile.name;
  });
  var skipped = 0;
  var refreshed = 0;

  return new Promise(function (resolve) {
    function next() {
      if (runId !== usageRefreshRunId) {
        usageRefreshRunning = false;
        resolve({ refreshed: refreshed, skipped: skipped });
        return;
      }
      if (queue.length === 0) {
        saveProfileSnapshot();
        usageRefreshRunning = false;
        if (autoFiveHourScheduler) autoFiveHourScheduler.drain();
        if (!silent) {
          setStatus(skipped ? "Usage refreshed. Skipped profiles: " + skipped + "." : "All profile usage refreshed.");
        }
        resolve({ refreshed: refreshed, skipped: skipped });
        return;
      }
      var name = queue.shift();
      window.setTimeout(function () {
        invokeBackend("refresh_profile_usage", { profileName: name }).then(function (usage) {
          if (usage && runId === usageRefreshRunId) {
            applyProfileUsage(usage);
            refreshed += 1;
          }
          next();
        }).catch(function (error) {
          skipped += 1;
          console.warn("refresh_profile_usage failed", name, error);
          next();
        });
      }, 40);
    }

    next();
  }).catch(function (error) {
    usageRefreshRunning = false;
    throw error;
  });
}

function startUsageRefreshTimer() {
  if (isMockMode || usageRefreshTimer) return;
  usageRefreshTimer = window.setInterval(function () {
    refreshAllProfileUsage(true, { silent: true });
  }, 60 * 1000);
}

function updateNativeTray() {
  var invoke = getTauriInvoke();
  if (!invoke) return;
  invoke("update_tray_profiles", {
    profiles: profiles.map(function (profile) {
      return {
        name: profile.name,
        active: profile.active
      };
    })
  }).catch(function (error) {
    console.warn("update_tray_profiles failed", error);
  });
}

function invokeWindowCommand(commandName) {
  var invoke = getTauriInvoke();
  if (!invoke) {
    showToast(commandName === "close_window" ? "Close window" : "Minimize window");
    return;
  }
  invoke(commandName).catch(function (error) {
    console.warn(commandName + " failed", error);
  });
}

function showDetachedSidePanel(title, bodyHtml, deferOpen) {
  if (deferOpen) els.appShell.classList.remove("panel-open");
  els.appShell.classList.toggle("panel-config", sideMode === "config");
  els.appShell.classList.toggle("panel-more", sideMode === "more");
  els.appShell.classList.toggle("panel-store", sideMode === "store");
  els.closeSideIcon.src = layoutMode === "single" ? "./assets/close-down.png" : "./assets/close.png";
  els.sideTitle.textContent = title;
  if (sideMode !== "store") els.sideBody.style.width = "";
  els.sideBody.innerHTML = bodyHtml;
  els.sidePanel.setAttribute("aria-hidden", "false");
  sideOpen = true;
  syncAdminWorkspaceState(true);
  syncTauriWindowSize(getLayoutWindowWidth());
  updateProfileHintVisibility();
  if (!deferOpen) {
    window.requestAnimationFrame(function () {
      els.appShell.classList.add("panel-open");
    });
  }
  setStatus(title + " panel opened.");
}

function beginWindowDrag(event) {
  if (event.button !== 0) return;
  if (event.target.closest("button, input, textarea, select, a, .top-actions, .window-actions, .side-panel, .admin-inventory-panel, .confirm-layer")) return;
  event.preventDefault();
  function fallbackDrag() {
    var invoke = getTauriInvoke();
    if (!invoke) return;
    invoke("start_window_drag").catch(function (error) {
      console.warn("start_window_drag failed", error);
      setStatus("Window drag failed: " + String(error));
    });
  }

  if (window.__TAURI__ && window.__TAURI__.window && window.__TAURI__.window.getCurrentWindow) {
    window.__TAURI__.window.getCurrentWindow().startDragging().catch(function (error) {
      console.warn("frontend startDragging failed", error);
      fallbackDrag();
    });
    return;
  }

  fallbackDrag();
}

function renderConfig() {
  sideMode = "config";
  var profile = getSelectedProfile();
  if (!profile) {
    setStatus("No profile selected.");
    return;
  }
  var bodyHtml = "";
  if (layoutMode === "double") {
    els.sideControls.innerHTML = "";
    bodyHtml =
      '<section class="config-block">' +
      "<strong>" + escapeHtml(t("auth")) + "</strong>" +
      "<pre>" + escapeHtml(profile.authText) + "</pre>" +
      "</section>" +
      '<section class="config-block">' +
      "<strong>" + escapeHtml(t("config")) + "</strong>" +
      "<pre>" + escapeHtml(profile.configText) + "</pre>" +
      "</section>";
  } else {
    var activeText = configViewMode === "config" ? profile.configText : profile.authText;
    bodyHtml =
      '<section class="config-block single-config-block">' +
      "<pre>" + escapeHtml(activeText) + "</pre>" +
      "</section>";
    els.sideControls.innerHTML =
      '<div class="segment-control ' + (configViewMode === "config" ? "config-active" : "auth-active") + '" role="tablist" aria-label="' + escapeHtml(t("config")) + '">' +
      '<button class="' + (configViewMode === "auth" ? "active" : "") + '" data-config-view="auth" role="tab" aria-selected="' + (configViewMode === "auth" ? "true" : "false") + '">' + escapeHtml(t("auth")) + "</button>" +
      '<button class="' + (configViewMode === "config" ? "active" : "") + '" data-config-view="config" role="tab" aria-selected="' + (configViewMode === "config" ? "true" : "false") + '">' + escapeHtml(t("config")) + "</button>" +
      "</div>";
  }
  showDetachedSidePanel(layoutMode === "double" ? t("config") + " - " + profile.name : t("config"), bodyHtml);
}

function renderProfileConfig(profileName) {
  fiveHourConfirmedProfiles.delete(profileName);
  selectedName = profileName;
  selectedVisualName = profileName;
  scheduleSelectionClear();
  renderProfiles();
  renderConfig();
}

function adminProductLabel(products) {
  var list = Array.isArray(products) ? products : [];
  if (list.length >= 2) return currentLanguage === "zh-cn" ? "月度 + 周额度" : "Monthly + weekly";
  if (list[0] === "monthly_30d") return currentLanguage === "zh-cn" ? "月度" : "Monthly";
  if (list[0] === "weekly_100_3d") return currentLanguage === "zh-cn" ? "周额度" : "Weekly";
  return "-";
}

function truncateAdminProfileName(value) {
  return Array.from(String(value || "-")).slice(0, 12).join("");
}

function getFilteredAdminInventory() {
  var productId = adminInventoryFilter === "weekly" ? "weekly_100_3d" : "monthly_30d";
  return adminInventory.filter(function (item) {
    return Array.isArray(item.eligibleProducts) && item.eligibleProducts.indexOf(productId) >= 0;
  });
}

function renderAdminInventoryHtml() {
  var visibleInventory = getFilteredAdminInventory();
  if (!visibleInventory.length) {
    return '<p class="admin-inventory-empty">' + escapeHtml(t("inventoryEmpty")) + "</p>";
  }
  return visibleInventory.map(function (item) {
    var usage = typeof item.usageAtUpload === "number" ? formatUsagePercent(item.usageAtUpload) : "-";
    var itemId = String(item.id || "");
    var fullName = String(item.displayName || "-");
    var selected = itemId && itemId === selectedAdminInventoryId;
    var hasUsage = typeof item.usageAtUpload === "number";
    var usageTone = hasUsage && item.usageAtUpload < 35 ? "warn" : (hasUsage && item.usageAtUpload < 70 ? "mid" : "");
    return '<article class="admin-inventory-item' + (selected ? " selected" : "") + '" data-admin-inventory-id="' + escapeHtml(itemId) + '">' +
      '<div class="admin-inventory-card-header"><strong title="' + escapeHtml(fullName) + '">' + escapeHtml(truncateAdminProfileName(fullName)) + '</strong><span class="admin-state-badge">' + escapeHtml(item.status || "-") + "</span></div>" +
      '<div class="admin-inventory-lines">' +
      '<div><span>' + escapeHtml(t("eligibleProduct")) + '</span><b>' + escapeHtml(adminProductLabel(item.eligibleProducts)) + "</b></div>" +
      '<div><span>' + escapeHtml(t("plan")) + '</span><b>' + escapeHtml(formatPlan(item.plan)) + "</b></div>" +
      '<div><span>' + escapeHtml(t("email")) + '</span><b>' + escapeHtml(item.email || "-") + "</b></div>" +
      '<div><span>' + escapeHtml(t("reset")) + '</span><b>' + escapeHtml(item.resetAtUpload || "-") + "</b></div>" +
      '<div class="admin-reset-row"><span>' + escapeHtml(t("credentialReset")) + '</span><b class="' + (item.credentialResetRequired ? "required" : "clear") + '">' + escapeHtml(item.credentialResetRequired ? t("yes") : t("no")) + "</b></div>" +
      "</div>" +
      '<div class="admin-usage-row"><span>' + escapeHtml(t("primaryLeft")) + '</span><strong class="' + usageTone + '">' + escapeHtml(usage) + "</strong></div>" +
      "</article>";
  }).join("");
}

function getSelectedAdminInventory() {
  for (var i = 0; i < adminInventory.length; i += 1) {
    if (String(adminInventory[i].id || "") === selectedAdminInventoryId) return adminInventory[i];
  }
  return null;
}

function updateAdminSelectionUi() {
  var cards = els.adminInventoryGrid.querySelectorAll("[data-admin-inventory-id]");
  for (var i = 0; i < cards.length; i += 1) {
    cards[i].classList.toggle("selected", cards[i].getAttribute("data-admin-inventory-id") === selectedAdminInventoryId);
  }
  var selected = getSelectedAdminInventory();
  var label = els.sideBody.querySelector("[data-admin-selection-label]");
  if (label) label.textContent = selected ? (selected.displayName || "-") : t("noInventorySelection");
  var transferButton = els.sideBody.querySelector('[data-admin-action="transfer"]');
  if (transferButton) transferButton.disabled = !selected;
}

function renderAdminInventoryPanel() {
  if (!els.adminInventoryGrid) return;
  var visibleInventory = getFilteredAdminInventory();
  if (selectedAdminInventoryId && !visibleInventory.some(function (item) {
    return String(item.id || "") === selectedAdminInventoryId;
  })) selectedAdminInventoryId = "";
  els.adminInventoryTitle.textContent = t("inventoryPreview");
  els.adminInventoryCount.textContent = String(visibleInventory.length);
  els.adminInventoryRefreshBtn.title = t("refreshInventory");
  els.adminInventoryRefreshBtn.setAttribute("aria-label", t("refreshInventory"));
  els.adminFilterWeeklyBtn.textContent = t("filterWeekly");
  els.adminFilterMonthlyBtn.textContent = t("filterMonthly");
  els.adminFilterWeeklyBtn.classList.toggle("active", adminInventoryFilter === "weekly");
  els.adminFilterMonthlyBtn.classList.toggle("active", adminInventoryFilter === "monthly");
  els.adminFilterWeeklyBtn.setAttribute("aria-selected", adminInventoryFilter === "weekly" ? "true" : "false");
  els.adminFilterMonthlyBtn.setAttribute("aria-selected", adminInventoryFilter === "monthly" ? "true" : "false");
  els.adminInventoryGrid.innerHTML = renderAdminInventoryHtml();
  updateAdminSelectionUi();
}

function renderAdminToolboxHtml() {
  if (!adminUnlocked || layoutMode !== "double") return "";
  return '<section class="admin-toolbox">' +
    '<div class="admin-toolbox-heading"><strong>' + escapeHtml(t("adminToolbox")) + '</strong>' +
    '<button class="admin-logout-icon" data-admin-action="logout" title="' + escapeHtml(t("adminLogout")) + '" aria-label="' + escapeHtml(t("adminLogout")) + '"><img src="./assets/logout.png" alt="" /></button></div>' +
    '<div class="admin-tool-row">' +
    '<button class="admin-upload-button" data-admin-action="upload">' + escapeHtml(t("uploadSelected")) + "</button>" +
    '<button data-admin-action="transfer"' + (getSelectedAdminInventory() ? "" : " disabled") + '>' + escapeHtml(t("transferPool")) + "</button>" +
    "</div>" +
    '<div class="admin-selection-context"><span>' + escapeHtml(t("selectedInventory")) + '</span><strong data-admin-selection-label>' +
    escapeHtml(getSelectedAdminInventory() ? (getSelectedAdminInventory().displayName || "-") : t("noInventorySelection")) + "</strong></div>" +
    "</section>";
}

function clearAdminSession(message) {
  adminUnlocked = false;
  adminToken = "";
  adminSessionExpiresAt = "";
  adminInventory = [];
  selectedAdminInventoryId = "";
  syncAdminWorkspaceState(true);
  renderProfiles();
  if (sideMode === "more" && sideOpen) renderMore();
  if (message) setStatus(message);
}

function refreshAdminInventory(silent) {
  if (!adminToken) return Promise.resolve();
  if (!silent) setStatus(currentLanguage === "zh-cn" ? "正在刷新云端库存..." : "Refreshing cloud inventory...");
  return invokeBackend("cloudbase_admin_inventory", { adminToken: adminToken }).then(function (result) {
    adminInventory = result && Array.isArray(result.items) ? result.items : [];
    if (selectedAdminInventoryId && !getSelectedAdminInventory()) selectedAdminInventoryId = "";
    renderAdminInventoryPanel();
    if (sideMode === "more" && sideOpen) renderMore();
    if (!silent) setStatus(currentLanguage === "zh-cn" ? "云端库存已刷新。" : "Cloud inventory refreshed.");
  }).catch(function (error) {
    var message = error && error.message ? error.message : String(error);
    if (/expired|失效/i.test(message)) clearAdminSession(message);
    throw error;
  });
}

function requestAdminUnlock() {
  if (adminUnlocked) {
    showToast(currentLanguage === "zh-cn" ? "管理员工具箱已解锁" : "Administrator tools are unlocked");
    return;
  }
  invokeBackend("get_cloudbase_status").then(function (status) {
    cloudbaseState = status || cloudbaseState;
    if (!status || !status.configured) {
      throw new Error(t("cloudNotConfigured") + (status && status.configPath ? ": " + status.configPath : ""));
    }
    return showPromptDialog(
      t("adminPasswordTitle"),
      t("adminPasswordMessage"),
      t("adminPasswordPlaceholder"),
      "password"
    );
  }).then(function (password) {
    if (password === null || !String(password).length) return null;
    setStatus(currentLanguage === "zh-cn" ? "正在验证管理员身份..." : "Verifying administrator...");
    return invokeBackend("cloudbase_admin_login", { password: password });
  }).then(function (result) {
    if (!result) return;
    adminToken = result.adminToken || "";
    adminSessionExpiresAt = result.expiresAt || "";
    adminUnlocked = !!adminToken;
    cloudbaseIdentity = { uid: result.uid || "" };
    forceSingleColumn = false;
    renderProfiles();
    renderMore();
    setStatus(currentLanguage === "zh-cn" ? "管理员工具箱已解锁。" : "Administrator toolbox unlocked.");
    return refreshAdminInventory(true);
  }).catch(function (error) {
    var message = error && error.message ? error.message : String(error);
    setStatus("Failed: " + message);
    showToast(("Failed: " + message).slice(0, 96));
  });
}

function uploadSelectedProfileToCloud() {
  var profile = getSelectedProfile();
  if (!profile || !adminToken) {
    showToast(currentLanguage === "zh-cn" ? "请先选择一张档案卡片" : "Select a profile first");
    return;
  }
  showPoolChoiceDialog(profile.name).then(function (pool) {
    if (!pool) return;
    var eligibleProducts = pool === "weekly" ? ["weekly_100_3d"] : ["monthly_30d"];
    setStatus(currentLanguage === "zh-cn" ? "正在加密并上传配置档案..." : "Encrypting and uploading profile...");
    return invokeBackend("cloudbase_admin_upload_profile", {
      adminToken: adminToken,
      profileName: profile.name,
      eligibleProducts: eligibleProducts,
      notes: null
    }).then(function () {
      showToast(currentLanguage === "zh-cn" ? "配置档案已加入云端库存" : "Profile added to cloud inventory");
      return refreshAdminInventory(false);
    });
  }).catch(function (error) {
    var message = error && error.message ? error.message : String(error);
    setStatus("Failed: " + message);
    showToast(("Failed: " + message).slice(0, 96));
  });
}

function transferSelectedInventoryPool() {
  var item = getSelectedAdminInventory();
  if (!item || !adminToken) {
    showToast(currentLanguage === "zh-cn" ? "请先选择一张云端库存卡片" : "Select a cloud inventory card first");
    return;
  }
  var products = Array.isArray(item.eligibleProducts) ? item.eligibleProducts : [];
  var targetProductId;
  if (products.length === 1 && products[0] === "monthly_30d") targetProductId = "weekly_100_3d";
  else if (products.length === 1 && products[0] === "weekly_100_3d") targetProductId = "monthly_30d";
  else targetProductId = adminInventoryFilter === "weekly" ? "monthly_30d" : "weekly_100_3d";
  setStatus(currentLanguage === "zh-cn" ? "正在转换库存池..." : "Moving profile to the other pool...");
  invokeBackend("cloudbase_admin_move_profile_pool", {
    adminToken: adminToken,
    profileId: item.id,
    targetProductId: targetProductId
  }).then(function () {
    showToast(currentLanguage === "zh-cn" ? "库存池已转换" : "Inventory pool updated");
    return refreshAdminInventory(false);
  }).catch(function (error) {
    var message = error && error.message ? error.message : String(error);
    setStatus("Failed: " + message);
    showToast(("Failed: " + message).slice(0, 96));
  });
}

function logoutAdmin() {
  var token = adminToken;
  clearAdminSession(currentLanguage === "zh-cn" ? "已退出管理员模式。" : "Administrator mode closed.");
  if (!token) return;
  invokeBackend("cloudbase_admin_logout", { adminToken: token }).catch(function (error) {
    console.warn("cloudbase_admin_logout failed", error);
  });
}

function renderMore() {
  sideMode = "more";
  els.sideControls.innerHTML = "";
  var bodyHtml =
    '<section class="sidebar-actions">' +
    '<button data-more-action="save-current">' + escapeHtml(t("saveCurrent")) + "</button>" +
    '<button data-more-action="toggle-auto-five-hour" aria-pressed="' + autoFiveHourEnabled + '" title="' + escapeHtml(t("autoFiveHourHint")) + '">' + escapeHtml(t("autoFiveHour")) + (currentLanguage === "zh-cn" ? "：" : ": ") + escapeHtml(t(autoFiveHourEnabled ? "on" : "off")) + "</button>" +
    '<button data-more-action="toggle-sync-local-projects" aria-pressed="' + syncLocalProjectsEnabled +
    '" title="' + escapeHtml(t("syncLocalProjectsHint")) + '"' + (syncLocalProjectsSettingBusy ? " disabled" : "") + '>' +
    escapeHtml(t("syncLocalProjects")) + (currentLanguage === "zh-cn" ? "：" : ": ") +
    escapeHtml(t(syncLocalProjectsEnabled ? "on" : "off")) + "</button>" +
    '<div class="sidebar-action-row">' +
    '<button data-more-action="toggle-email">' + escapeHtml(showEmails ? t("emailHide") : t("emailShow")) + "</button>" +
    '<button data-more-action="toggle-language">' + escapeHtml(t("languageButton")) + "</button>" +
    "</div>" +
    '<div class="sidebar-action-row">' +
    '<button data-more-action="open-codex">' + escapeHtml(t("codexDir")) + "</button>" +
    '<button data-more-action="open-data">' + escapeHtml(t("dataDir")) + "</button>" +
    '</div>' +
    '<button data-more-action="open-app">' + escapeHtml(t("openApp")) + "</button>" +
    '<button class="danger-text" data-more-action="hard-kill">' + escapeHtml(t("hardKill")) + "</button>" +
    '<div class="sidebar-action-row">' +
    '<button data-more-action="toggle-tibo-post" aria-pressed="' + showTiboPost + '">' + escapeHtml(t("showTiboRecentPost")) + ": " + escapeHtml(t(showTiboPost ? "on" : "off")) + "</button>" +
    '<button data-more-action="toggle-floating-bar" aria-pressed="' + floatingBarEnabled + '">' + escapeHtml(t("floatingBar")) + (currentLanguage === "zh-cn" ? "：" : ": ") + escapeHtml(t(floatingBarEnabled ? "on" : "off")) + "</button>" +
    '</div>' +
    renderAdminToolboxHtml() +
    "</section>";
  showDetachedSidePanel("Ver. " + appVersion, bodyHtml);
}

function fitStoreScrollArea(storeFrame) {
  if (!storeFrame || !storeFrame.isConnected) return;
  var panelStyle = window.getComputedStyle(els.sidePanel);
  var contentWidth = Math.floor(
    els.sidePanel.clientWidth -
    parseFloat(panelStyle.paddingLeft || "0") -
    parseFloat(panelStyle.paddingRight || "0")
  );
  if (contentWidth <= 0) return;
  storeFrame.style.width = contentWidth + "px";
  els.sideBody.style.width = contentWidth + 14 + "px";
}

function renderStore() {
  sideMode = "store";
  els.sideControls.innerHTML = "";
  var existingFrame = document.getElementById("storeCloudFrame");
  if (sideOpen && existingFrame && storePageHtml) {
    if (existingFrame.contentWindow) {
      existingFrame.contentWindow.postMessage({ type: "codex-switch-store-language", language: currentLanguage, noticeExpanded: storeNoticeExpanded }, "*");
    }
    updateOpenStoreCatalog();
    return;
  }
  if (storePanelResizeObserver) {
    storePanelResizeObserver.disconnect();
    storePanelResizeObserver = null;
  }
  els.sideBody.style.width = "";
  if (!storePageHtml) {
    showDetachedSidePanel(t("monthlyAccess"),
      '<section class="store-panel-content"><p class="store-notice">' +
      escapeHtml(currentLanguage === "zh-cn" ? "正在加载云端商品页面；若持续显示，请检查云端服务。" : "Loading the cloud store page. Check the cloud service if this persists.") +
      "</p></section>");
    return;
  }
  showDetachedSidePanel(t("monthlyAccess"),
    '<iframe id="storeCloudFrame" class="store-cloud-frame" title="Store" sandbox="allow-scripts"></iframe>', true);
  var storeFrame = document.getElementById("storeCloudFrame");
  fitStoreScrollArea(storeFrame);
  if (typeof ResizeObserver !== "undefined") {
    storePanelResizeObserver = new ResizeObserver(function () {
      fitStoreScrollArea(storeFrame);
    });
    storePanelResizeObserver.observe(els.sidePanel);
  }
  storeFrame.addEventListener("load", function () {
    if (storeFrame.contentWindow) {
      storeFrame.contentWindow.postMessage({ type: "codex-switch-store-language", language: currentLanguage, noticeExpanded: storeNoticeExpanded }, "*");
      updateOpenStoreCatalog();
    }
  });
  storeFrame.srcdoc = storePageHtml;
  return;
  var monthlyAvailable = isStoreProductAvailable("monthly");
  var weeklyAvailable = isStoreProductAvailable("weekly");
  if (!isStoreProductAvailable(selectedStoreProduct)) {
    if (monthlyAvailable) selectedStoreProduct = "monthly";
    else if (weeklyAvailable) selectedStoreProduct = "weekly";
  }
  var monthlySelected = selectedStoreProduct === "monthly" && monthlyAvailable;
  var weeklySelected = selectedStoreProduct === "weekly" && weeklyAvailable;
  var selectedProduct = getStoreProduct(selectedStoreProduct);
  var selectedAvailable = isStoreProductAvailable(selectedProduct.id);
  var bodyHtml =
    '<section class="store-panel-content">' +
    '  <div class="store-plan-grid">' +
    '    <section class="store-plan-card store-plan-select' + (monthlySelected ? " selected" : "") + (monthlyAvailable ? "" : " unavailable") + '" data-store-product="monthly" role="button" tabindex="' + (monthlyAvailable ? "0" : "-1") + '" aria-disabled="' + (monthlyAvailable ? "false" : "true") + '" aria-label="Monthly plan">' +
    '      <div class="store-plan-top">' +
    '        <div class="store-plan-title-row"><strong>' + escapeHtml(t("monthlyPass")) + '</strong><span class="' + (monthlyAvailable ? "available" : "unavailable") + '">' + escapeHtml(storeAvailabilityLabel("monthly")) + "</span></div>" +
    '        <div class="store-price-row"><b>' + storeProducts.monthly.price + "</b><em>" + escapeHtml(t("perMonth")) + "</em></div>" +
    "      </div>" +
    '      <ul class="store-feature-list">' +
    '        <li><span>&#10003;</span>' + escapeHtml(t("monthlyFeatureAccess")) + "</li>" +
    '        <li><span>&#10003;</span>' + escapeHtml(t("monthlyFeaturePlus")) + "</li>" +
    "      </ul>" +
    "    </section>" +
    '    <section class="store-plan-card store-plan-card-secondary store-plan-select' + (weeklySelected ? " selected" : "") + (weeklyAvailable ? "" : " unavailable") + '" data-store-product="weekly" role="button" tabindex="' + (weeklyAvailable ? "0" : "-1") + '" aria-disabled="' + (weeklyAvailable ? "false" : "true") + '" aria-label="Weekly quota plan">' +
    '      <div class="store-plan-top">' +
    '        <div class="store-plan-title-row"><strong>' + escapeHtml(t("weeklyPass")) + '</strong><span class="' + (weeklyAvailable ? "available" : "unavailable") + '">' + escapeHtml(storeAvailabilityLabel("weekly")) + "</span></div>" +
    '        <div class="store-price-row"><b>' + storeProducts.weekly.price + "</b><em>" + escapeHtml(t("weeklyPriceUnit")) + "</em></div>" +
    "      </div>" +
    '      <ul class="store-feature-list">' +
    '        <li><span>&#10003;</span>' + escapeHtml(t("weeklyFeatureAccess")) + "</li>" +
    '        <li><span>&#10003;</span>' + escapeHtml(t("monthlyFeaturePlus")) + "</li>" +
    "      </ul>" +
    "    </section>" +
    "  </div>" +
    '  <p class="store-notice">' + escapeHtml(t("storeNotice")) + "</p>" +
    '  <section class="store-checkout-card" aria-label="Checkout">' +
    '    <div class="store-summary">' +
    '      <div class="store-total"><span>' + escapeHtml(t("totalDue")) + "</span><b>" + selectedProduct.price + "</b></div>" +
    "    </div>" +
    '    <button class="store-pay-button" type="button" data-store-action="pay" data-store-product="' + escapeHtml(selectedProduct.id) + '"' + (selectedAvailable ? "" : " disabled") + '>' + escapeHtml(selectedAvailable ? t("proceedPayment") : (storeInventoryLoaded ? t("soldOut") : t("checkingStock"))) + "</button>" +
    "  </section>" +
    "</section>";
  showDetachedSidePanel(t("monthlyAccess"), bodyHtml);
}

window.addEventListener("message", function (event) {
  var storeFrame = document.getElementById("storeCloudFrame");
  if (!storeFrame || !sideOpen || sideMode !== "store" || event.source !== storeFrame.contentWindow) return;
  if (!event.data) return;
  if (event.data.type === "codex-switch-store-resize") {
    var pageHeight = Number(event.data.height);
    if (Number.isFinite(pageHeight) && pageHeight >= 100 && pageHeight <= 1500) {
      var previousHeight = storeFrame.offsetHeight;
      var wasScrolling = els.sideBody.scrollHeight > els.sideBody.clientHeight + 1;
      storeFrame.style.height = Math.ceil(pageHeight) + "px";
      if (event.data.reason === "notice" && wasScrolling) {
        els.sideBody.scrollTop += Math.ceil(pageHeight) - previousHeight;
      }
      if (!els.appShell.classList.contains("panel-open")) {
        window.requestAnimationFrame(function () {
          if (sideOpen && sideMode === "store" && storeFrame.isConnected) {
            els.appShell.classList.add("panel-open");
          }
        });
      }
    }
    return;
  }
  if (event.data.type === "codex-switch-store-notice-expanded") {
    storeNoticeExpanded = true;
    saveBooleanSetting(storeNoticeExpandedStorageKey, true);
    return;
  }
  if (event.data.type !== "codex-switch-store-pay") return;
  var productId = Object.keys(storeProducts).find(function (key) {
    return storeProducts[key].cloudProductId === event.data.productId;
  });
  if (productId && isStoreProductAvailable(productId)) startStorePayment(productId);
});

function switchProfile(name, fromTray) {
  if (profileSwitchRunning) return;
  if (!name) {
    setStatus("No profile selected.");
    return;
  }
  if (!fromTray) {
    showConfirmDialog(
      "Activate Profile",
      "ChatGPT/Codex will stop, this profile will be deployed, then the app will restart."
    ).then(function (confirmed) {
      if (confirmed) switchProfile(name, true);
    });
    return;
  }
  selectedName = name;
  profileSwitchRunning = true;
  fiveHourConfirmedProfiles.delete(name);
  selectedVisualName = name;
  scheduleSelectionClear();
  renderProfiles();
  runBackendAction(
    "switch_profile",
    { profileName: name },
    "Stopping ChatGPT/Codex and deploying " + name + "...",
    "Switched to profile '" + name + "' and started ChatGPT/Codex."
  ).then(function () {
    showToast(fromTray ? "Tray switch: " + name : "Switched to " + name);
  }).catch(function () {
    // runBackendAction already displays the failure.
  }).finally(function () {
    profileSwitchRunning = false;
  });
}

function startLoginWatch() {
  if (loginWatchTimer) {
    window.clearInterval(loginWatchTimer);
  }
  var runId = loginWatchRunId + 1;
  loginWatchRunId = runId;
  loginStableSignature = "";
  loginStableCount = 0;
  setLoginWatchAbortVisible(true);
  setStatus(t("loginWaiting"));
  loginWatchTimer = window.setInterval(function () {
    invokeBackend("get_login_state")
      .then(function (state) {
        if (runId !== loginWatchRunId) return;
        setStatus(state.message);
        if (state.terminal) {
          stopLoginWatch(state.message || "Login stopped. No new profile was saved.");
          if (state.failed) showToast("Login failed");
          return;
        }
        if (!state.ready || !state.signature) {
          loginStableSignature = "";
          loginStableCount = 0;
          return;
        }
        if (state.signature === loginStableSignature) {
          loginStableCount += 1;
        } else {
          loginStableSignature = state.signature;
          loginStableCount = 1;
        }
        if (loginStableCount < 2) return;
        if (loginWatchTimer) {
          window.clearInterval(loginWatchTimer);
          loginWatchTimer = null;
        }
        setLoginWatchAbortVisible(false);
        runBackendAction(
          "finish_login_new_account",
          {},
          "Saving new login profile...",
          "New login profile saved."
        ).then(function (snapshot) {
          var active = snapshot && snapshot.active ? snapshot.active : "profile";
          selectedName = active;
          selectedVisualName = active;
          scheduleSelectionClear();
          showToast("Saved " + active);
          refreshOneProfileUsage(active);
        }).catch(function () {
          if (runId === loginWatchRunId) {
            setLoginWatchAbortVisible(false);
          }
        });
      })
      .catch(function (error) {
        if (runId !== loginWatchRunId) return;
        setStatus("Login watch failed: " + String(error));
      });
  }, 1000);
}

function loginNewAccount() {
  showConfirmDialog(
    t("newProfileLoginTitle"),
    t("newProfileLoginMessage")
  ).then(function (confirmed) {
    if (!confirmed) return;
    runBackendAction(
      "login_new_account",
      {},
      "Preparing new login flow...",
      "Login flow started."
    ).then(function () {
      closeSide();
      startLoginWatch();
    });
  });
}

function escapeHtml(value) {
  return String(value)
    .replace(/&/g, "&amp;")
    .replace(/</g, "&lt;")
    .replace(/>/g, "&gt;")
    .replace(/"/g, "&quot;");
}

document.getElementById("refreshBtn").onclick = function () {
  loadProfiles().then(function () {
    return refreshAllProfileUsage(true);
  }).then(function (result) {
    var skipped = result && Number(result.skipped || 0);
    showToast(skipped ? "Usage refreshed with " + skipped + " failed profile(s)" : "All usage refreshed");
  });
};

document.getElementById("switchBtn").onclick = function () {
  var profile = getProfileByName(selectedVisualName);
  if (!profile || profile.active) return;
  switchProfile(profile.name, false);
};

document.getElementById("loginBtn").onclick = function () {
  loginNewAccount();
};

document.getElementById("deleteBtn").onclick = function () {
  var profile = getProfileByName(selectedVisualName);
  if (!profile) return;
  showConfirmDialog(
    "Delete Profile",
    "Profile files will be moved to trash. The current .codex files will not be deleted."
  ).then(function (confirmed) {
    if (!confirmed) return;
    runBackendAction(
      "delete_profile",
      { profileName: profile.name },
      "Deleting profile " + profile.name + "...",
      "Deleted profile: " + profile.name
    ).then(function () {
      delete usageCache[profile.name];
      closeSide();
      showToast("Deleted " + profile.name);
    });
  });
};

document.getElementById("storeBtn").onclick = function () {
  renderStore();
  refreshStoreCatalog().catch(function (error) {
    var message = error && error.message ? error.message : String(error);
    setStatus("Failed: " + message);
  });
};

document.getElementById("moreBtn").onclick = function () {
  syncTauriWindowSize();
  renderMore();
};
els.columnToggleBtn.onclick = function () {
  if (profiles.length < 4) return;
  if (sideOpen) closeSide();
  forceSingleColumn = !forceSingleColumn;
  selectedVisualName = "";
  Array.from(els.profileList.querySelectorAll(".profile-card.selected")).forEach(function (card) {
    card.classList.remove("selected");
  });
  applyProfileLayoutState();
  updateActionButtons();
  setStatus(forceSingleColumn ? "Single-column layout enabled." : "Two-column layout restored.");
};

document.getElementById("minimizeWindowBtn").onclick = function () {
  invokeWindowCommand("minimize_window");
};

document.getElementById("closeWindowBtn").onclick = function () {
  invokeWindowCommand("close_window");
};

els.abortLoginWatchBtn.onclick = function () {
  invokeBackend("abort_login_new_account")
    .then(function (result) {
      stopLoginWatch(result && result.message ? result.message : "Login cancelled. No new profile was saved.");
      showToast("Login cancelled");
    })
    .catch(function (error) {
      setStatus("Failed to abort login: " + String(error));
    });
};

els.paymentCloseBtn.onclick = function () { closePaymentLayer(); };
els.paymentCancelBtn.onclick = function () { closePaymentLayer(); };
els.paymentRedeemCodeBtn.onclick = function () {
  showToast(currentLanguage === "zh-cn" ? "兑换码功能即将上线" : "Code redemption is coming soon");
};

els.closeSideBtn.onclick = closeSide;

function cancelAdminVersionHold() {
  if (adminVersionHoldTimer) {
    window.clearTimeout(adminVersionHoldTimer);
    adminVersionHoldTimer = null;
  }
}

els.sideTitle.onpointerdown = function (event) {
  if (event.button !== 0 || sideMode !== "more" || !sideOpen) return;
  cancelAdminVersionHold();
  adminVersionHoldTimer = window.setTimeout(function () {
    adminVersionHoldTimer = null;
    requestAdminUnlock();
  }, 5000);
};
els.sideTitle.onpointerup = cancelAdminVersionHold;
els.sideTitle.onpointercancel = cancelAdminVersionHold;
els.sideTitle.onpointerleave = cancelAdminVersionHold;

els.sideControls.onclick = function (event) {
  var button = event.target.closest("[data-config-view]");
  if (!button) return;
  configViewMode = button.getAttribute("data-config-view") === "config" ? "config" : "auth";
  renderConfig();
};
els.sideBody.onclick = function (event) {
  var adminButton = event.target.closest("[data-admin-action]");
  if (adminButton) {
    var adminAction = adminButton.getAttribute("data-admin-action");
    if (adminAction === "upload") uploadSelectedProfileToCloud();
    if (adminAction === "refresh") {
      refreshAdminInventory(false).catch(function (error) {
        var message = error && error.message ? error.message : String(error);
        setStatus("Failed: " + message);
      });
    }
    if (adminAction === "transfer") transferSelectedInventoryPool();
    if (adminAction === "logout") logoutAdmin();
    return;
  }
  var storeProductButton = event.target.closest("[data-store-product]");
  if (storeProductButton && !storeProductButton.hasAttribute("data-store-action")) {
    var nextStoreProduct = storeProductButton.getAttribute("data-store-product") || "monthly";
    if (!isStoreProductAvailable(nextStoreProduct)) {
      showToast(currentLanguage === "zh-cn" ? "当前套餐暂无可用库存" : "This product is out of stock");
      return;
    }
    selectedStoreProduct = nextStoreProduct;
    renderStore();
    return;
  }
  var storeButton = event.target.closest("[data-store-action]");
  if (storeButton) {
    var storeAction = storeButton.getAttribute("data-store-action");
    if (storeAction === "pay") {
      startStorePayment(storeButton.getAttribute("data-store-product"));
    }
    return;
  }
  var button = event.target.closest("[data-more-action]");
  if (!button) return;
  var action = button.getAttribute("data-more-action");
  if (action === "toggle-auto-five-hour") {
    setAutoFiveHourEnabled(!autoFiveHourEnabled);
    showToast(t("autoFiveHour") + ": " + t(autoFiveHourEnabled ? "on" : "off"));
    renderMore();
    return;
  }
  if (action === "toggle-sync-local-projects") {
    if (syncLocalProjectsSettingBusy) return;
    syncLocalProjectsSettingBusy = true;
    button.disabled = true;
    invokeBackend("set_sync_local_projects", { enabled: !syncLocalProjectsEnabled }).then(function (settings) {
      syncLocalProjectsEnabled = settings.syncLocalProjects === true;
      showToast(t("syncLocalProjects") + (currentLanguage === "zh-cn" ? "：" : ": ") + t(syncLocalProjectsEnabled ? "on" : "off"));
    }).catch(function (error) {
      console.warn("set_sync_local_projects failed", error);
      showToast(t("syncLocalProjectsSaveFailed"));
    }).finally(function () {
      syncLocalProjectsSettingBusy = false;
      if (sideOpen && sideMode === "more") renderMore();
    });
    return;
  }
  if (action === "toggle-email") {
    showEmails = !showEmails;
    renderProfiles();
    showToast(showEmails ? "Email visible" : "Email hidden");
    renderMore();
    return;
  }
  if (action === "toggle-language") {
    var nextLanguage = currentLanguage === "zh-cn" ? "en" : "zh-cn";
    setAppLanguage(nextLanguage).then(function () {
      showToast(nextLanguage === "zh-cn" ? "语言: 中文" : "Language: English");
    });
    return;
  }
  if (action === "toggle-tibo-post") {
    setTiboPostVisibility(!showTiboPost);
    showToast(t("showTiboRecentPost") + ": " + t(showTiboPost ? "on" : "off"));
    renderMore();
    return;
  }
  if (action === "toggle-floating-bar") {
    setFloatingBarEnabled(!floatingBarEnabled);
    showToast(t("floatingBar") + (currentLanguage === "zh-cn" ? "：" : ": ") + t(floatingBarEnabled ? "on" : "off"));
    renderMore();
    return;
  }
  runAfterSideClosed(function () {
  if (action === "save-current") {
    runBackendAction(
      "save_current_as_new",
      { profileName: null },
      "Saving current config...",
      "Current config saved."
    ).then(function (snapshot) {
      var active = snapshot && snapshot.active ? snapshot.active : "profile";
      selectedName = active;
      selectedVisualName = active;
      scheduleSelectionClear();
      showToast("Saved " + active);
      refreshOneProfileUsage(active);
    });
  }
  if (action === "open-app") {
    runBackendAction("open_chatgpt_codex", {}, "Opening ChatGPT/Codex...", "Opened ChatGPT/Codex.");
  }
  if (action === "open-data") {
    runBackendAction("open_data_dir", {}, "Opening data dir...", "Opened data dir.");
  }
  if (action === "open-codex") {
    runBackendAction("open_codex_dir", {}, "Opening Codex dir...", "Opened Codex dir.");
  }
  if (action === "hard-kill") {
    showConfirmDialog(
      "Hard Kill",
      "All ChatGPT/Codex processes will be forcefully stopped."
    ).then(function (confirmed) {
      if (!confirmed) return;
      runBackendAction("hard_kill_codex", {}, "Killing ChatGPT/Codex...", "Hard kill complete.");
    });
  }
  });
};

els.adminInventoryPanel.onclick = function (event) {
  var adminButton = event.target.closest("[data-admin-action]");
  if (adminButton && adminButton.getAttribute("data-admin-action") === "refresh") {
    refreshAdminInventory(false).catch(function (error) {
      var message = error && error.message ? error.message : String(error);
      setStatus("Failed: " + message);
    });
    return;
  }
  var filterButton = event.target.closest("[data-admin-inventory-filter]");
  if (filterButton && isAdminWorkspaceOpen()) {
    adminInventoryFilter = filterButton.getAttribute("data-admin-inventory-filter") === "weekly" ? "weekly" : "monthly";
    renderAdminInventoryPanel();
    return;
  }
  var card = event.target.closest("[data-admin-inventory-id]");
  if (!card || !isAdminWorkspaceOpen()) return;
  var itemId = card.getAttribute("data-admin-inventory-id") || "";
  selectedAdminInventoryId = selectedAdminInventoryId === itemId ? "" : itemId;
  updateAdminSelectionUi();
};

els.sideBody.onkeydown = function (event) {
  if (event.key !== "Enter" && event.key !== " ") return;
  var storeProductButton = event.target.closest("[data-store-product]");
  if (!storeProductButton || storeProductButton.hasAttribute("data-store-action")) return;
  if (!isStoreProductAvailable(storeProductButton.getAttribute("data-store-product"))) return;
  event.preventDefault();
  selectedStoreProduct = storeProductButton.getAttribute("data-store-product") || "monthly";
  renderStore();
};
function shouldBlockWorkspaceEvent(event) {
  return layoutMode === "single" && sideOpen && !event.target.closest(".side-panel, .confirm-layer, .payment-layer");
}

els.workspace.addEventListener("click", function (event) {
  if (!shouldBlockWorkspaceEvent(event)) return;
  event.preventDefault();
  event.stopPropagation();
  closeSide();
}, true);

els.workspace.addEventListener("wheel", function (event) {
  if (!shouldBlockWorkspaceEvent(event)) return;
  event.preventDefault();
  event.stopPropagation();
}, { capture: true, passive: false });

function canScrollInDirection(element, deltaY) {
  if (!element || Math.abs(deltaY) < 1) return false;
  if (element.scrollHeight <= element.clientHeight + 1) return false;
  if (deltaY < 0) return element.scrollTop > 0;
  return element.scrollTop + element.clientHeight < element.scrollHeight - 1;
}

els.sidePanel.addEventListener("wheel", function (event) {
  if (!(layoutMode === "single" && sideOpen)) return;
  var scroller = event.target.closest(".side-body");
  if (canScrollInDirection(scroller, event.deltaY)) {
    event.stopPropagation();
    return;
  }
  event.preventDefault();
  event.stopPropagation();
}, { capture: true, passive: false });

document.addEventListener("wheel", function (event) {
  if (!shouldBlockWorkspaceEvent(event)) return;
  event.preventDefault();
  event.stopPropagation();
}, { capture: true, passive: false });

document.addEventListener("touchmove", function (event) {
  if (!shouldBlockWorkspaceEvent(event)) return;
  event.preventDefault();
  event.stopPropagation();
}, { capture: true, passive: false });
function listenNativeTray() {
  if (!window.__TAURI__ || !window.__TAURI__.event || !window.__TAURI__.event.listen) return;
  window.__TAURI__.event.listen("tray-menu-event", function (event) {
    var payload = event.payload || {};
    if (payload.action === "login") {
      loginNewAccount();
    }
    if (payload.action === "switch" && typeof payload.index === "number" && profiles[payload.index]) {
      switchProfile(profiles[payload.index].name, true);
    }
  }).catch(function (error) {
    console.warn("tray-menu-event listen failed", error);
  });
}

function listenPanelOpenState() {
  if (!window.__TAURI__ || !window.__TAURI__.event || !window.__TAURI__.event.listen) return;
  window.__TAURI__.event.listen("side-panel-content", function (event) {
    var payload = event.payload || {};
    els.sideTitle.textContent = payload.title || "Panel";
    els.sideBody.innerHTML = payload.body_html || "";
  }).catch(function (error) {
    console.warn("side-panel-content listen failed", error);
  });
}

document.addEventListener("contextmenu", function (event) {
  event.preventDefault();
});
document.addEventListener("dragstart", function (event) {
  event.preventDefault();
});

listenNativeTray();
listenPanelOpenState();
startResetHistoryIndicator();
setupResetIndicatorLongPress();
startTiboPostTicker();
Promise.all([loadAppSettings(), loadAppVersion()]).then(function () {
  loadUsageCache();
  loadProfileSnapshot();
  return loadProfiles().catch(function (error) {
    setStatus("Profile load failed: " + String(error));
    renderProfiles();
  });
}).then(function () {
  initializeAutoFiveHour();
  startUsageRefreshTimer();
  refreshAllProfileUsage(false, { silent: true });
  return initializeCloudBase();
}).then(function () {
  return syncPurchasedProfiles({ silent: true });
}).then(function () {
  if (loadPendingOrderId()) startDeliveryPolling();
});
