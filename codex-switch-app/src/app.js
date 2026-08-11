var profiles = [];

var selectedName = "";
var selectedVisualName = "";
var selectedClearTimer = null;
var sideMode = "";
var sideOpen = false;
var configViewMode = "auth";
var layoutMode = "single";
var loginWatchTimer = null;
var loginStableSignature = "";
var loginStableCount = 0;
var initialUsageRefreshed = false;
var usageCache = {};
var baseWindowHeight = 740;
var statusExtraHeight = 0;
var mockProfileCount = getMockProfileCount();
var isMockMode = mockProfileCount > 0;

var els = {
  appShell: document.querySelector(".app-shell"),
  workspace: document.querySelector(".workspace"),
  profileList: document.getElementById("profileList"),
  profileHint: document.getElementById("profileHint"),
  statusbar: document.getElementById("statusbar"),
  switchBtn: document.getElementById("switchBtn"),
  deleteBtn: document.getElementById("deleteBtn"),
  sidePanel: document.getElementById("sidePanel"),
  sideTitle: document.getElementById("sideTitle"),
  sideControls: document.getElementById("sideControls"),
  sideBody: document.getElementById("sideBody"),
  closeSideBtn: document.getElementById("closeSideBtn"),
  closeSideIcon: document.getElementById("closeSideIcon"),
  confirmLayer: document.getElementById("confirmLayer"),
  confirmTitle: document.getElementById("confirmTitle"),
  confirmMessage: document.getElementById("confirmMessage"),
  confirmInput: document.getElementById("confirmInput"),
  confirmCancelBtn: document.getElementById("confirmCancelBtn"),
  confirmOkBtn: document.getElementById("confirmOkBtn"),
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

function updateActionButtons() {
  var visualProfile = getProfileByName(selectedVisualName);
  var canDelete = !!visualProfile;
  var canSwitch = !!visualProfile && !visualProfile.active;
  els.switchBtn.disabled = !canSwitch;
  els.deleteBtn.disabled = !canDelete;
}

function setStatus(text) {
  els.statusbar.textContent = text;
  console.log("[Codex Switch] " + text);
  updateStatusHeight();
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
      reset: "2026-08-18 16:28",
      primaryLeft: 69.0,
      secondaryLeft: null,
      active: false,
      authText: "{\n  \"account\": \"Mira Chen\",\n  \"email\": \"mira.chen@northmail.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Alex Morgan",
      email: "alex.morgan@orbitmail.dev",
      plan: "plus",
      reset: "2026-08-18 13:36",
      primaryLeft: 15.0,
      secondaryLeft: null,
      active: true,
      authText: "{\n  \"account\": \"Alex Morgan\",\n  \"email\": \"alex.morgan@orbitmail.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Noah Rivera",
      email: "noah.rivera@starlane.dev",
      plan: "plus",
      reset: "2026-08-18 14:19",
      primaryLeft: 74.0,
      secondaryLeft: null,
      active: false,
      authText: "{\n  \"account\": \"Noah Rivera\",\n  \"email\": \"noah.rivera@starlane.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    },
    {
      name: "Iris Walker",
      email: "iris.walker@bluepost.dev",
      plan: "team",
      reset: "2026-08-19 09:12",
      primaryLeft: 86.0,
      secondaryLeft: null,
      active: false,
      authText: "{\n  \"account\": \"Iris Walker\",\n  \"email\": \"iris.walker@bluepost.dev\"\n}",
      configText: "model = \"gpt-5-codex\"\napproval_policy = \"on-request\"\n"
    }
  ];
  return items.slice(0, count);
}

function showConfirmDialog(title, message) {
  return new Promise(function (resolve) {
    els.confirmTitle.textContent = title;
    els.confirmMessage.textContent = message;
    els.confirmInput.hidden = true;
    els.confirmInput.value = "";
    els.confirmLayer.setAttribute("aria-hidden", "false");
    els.confirmLayer.classList.add("show");
    els.confirmOkBtn.focus();

    function cleanup(result) {
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

function showPromptDialog(title, message, placeholder) {
  return new Promise(function (resolve) {
    els.confirmTitle.textContent = title;
    els.confirmMessage.textContent = message;
    els.confirmInput.hidden = false;
    els.confirmInput.value = "";
    els.confirmInput.placeholder = placeholder || "";
    els.confirmLayer.setAttribute("aria-hidden", "false");
    els.confirmLayer.classList.add("show");
    els.confirmInput.focus();

    function cleanup(result) {
      els.confirmLayer.classList.remove("show");
      els.confirmLayer.setAttribute("aria-hidden", "true");
      els.confirmInput.hidden = true;
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

function renderProfiles() {
  els.profileList.innerHTML = "";
  layoutMode = profiles.length >= 4 ? "double" : "single";
  els.appShell.classList.toggle("double-mode", layoutMode === "double");
  els.appShell.classList.toggle("single-mode", layoutMode === "single");
  syncTauriWindowSize(getLayoutWindowWidth());
  els.profileList.classList.toggle("single-column", profiles.length < 4);
  els.profileHint.hidden = profiles.length < 4;
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

    var hasUsage = typeof profile.primaryLeft === "number";
    var warn = hasUsage && profile.primaryLeft < 35;
    var mid = hasUsage && profile.primaryLeft >= 35 && profile.primaryLeft < 70;
    var usageTone = warn ? "warn" : (mid ? "mid" : "");
    var usageText = hasUsage ? profile.primaryLeft.toFixed(1) + "%" : "-";
    var usageWidth = hasUsage ? profile.primaryLeft : 0;
    card.innerHTML =
      '<div class="profile-card-header">' +
      '  <div class="profile-name">' +
      escapeHtml(profile.name) +
      "  </div>" +
      (profile.active ? '<span class="profile-badge">active</span>' : "") +
      "</div>" +
      '<div class="profile-lines">' +
      "  <div><span>Plan</span><b>" +
      escapeHtml(formatPlan(profile.plan)) +
      "</b></div>" +
      "  <div><span>Email</span><b>" +
      escapeHtml(profile.email) +
      "</b></div>" +
      "  <div><span>Reset</span><b>" +
      escapeHtml(profile.reset) +
      "</b></div>" +
      "</div>" +
      '<div class="usage-strip">' +
      '  <div class="usage-row"><span>Primary left</span><strong class="' +
      usageTone +
      '">' +
      usageText +
      "</strong></div>" +
      '  <div class="usage-bar"><div class="usage-fill ' +
      usageTone +
      '" style="width:' +
      usageWidth +
      '%"></div></div>' +
      "</div>" +
      "";

    card.onclick = function (event) {
      selectedName = this.getAttribute("data-profile");
      selectedVisualName = selectedName;
      scheduleSelectionClear();
      renderProfiles();
      if (sideMode === "config") renderConfig();
      setStatus("Selected profile: " + selectedName);
    };

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

function closeSide() {
  sideMode = "";
  sideOpen = false;
  var invoke = getTauriInvoke();
  els.appShell.classList.remove("panel-open");
  els.appShell.classList.remove("panel-config", "panel-more");
  els.sidePanel.setAttribute("aria-hidden", "true");
  els.sideControls.innerHTML = "";
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
  return layoutMode === "double" ? 680 : 365;
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

function applySnapshot(snapshot, statusText) {
  profiles = snapshot && Array.isArray(snapshot.profiles) ? snapshot.profiles : [];
  for (var p = 0; p < profiles.length; p += 1) {
    var cachedUsage = usageCache[profiles[p].name];
    if (!cachedUsage) continue;
    profiles[p].plan = cachedUsage.plan || profiles[p].plan;
    profiles[p].primaryLeft = typeof cachedUsage.primaryLeft === "number" ? cachedUsage.primaryLeft : profiles[p].primaryLeft;
    profiles[p].secondaryLeft = typeof cachedUsage.secondaryLeft === "number" ? cachedUsage.secondaryLeft : profiles[p].secondaryLeft;
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
  if (!isMockMode) updateNativeTray();
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
      showToast("Failed");
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

function applyProfileUsage(usage) {
  if (!usage || !usage.profileName) return;
  usageCache[usage.profileName] = {
    plan: usage.plan,
    primaryLeft: usage.primaryLeft,
    secondaryLeft: usage.secondaryLeft,
    reset: usage.reset
  };
  for (var i = 0; i < profiles.length; i += 1) {
    if (profiles[i].name !== usage.profileName) continue;
    profiles[i].plan = usage.plan || profiles[i].plan;
    profiles[i].primaryLeft = typeof usage.primaryLeft === "number" ? usage.primaryLeft : profiles[i].primaryLeft;
    profiles[i].secondaryLeft = typeof usage.secondaryLeft === "number" ? usage.secondaryLeft : profiles[i].secondaryLeft;
    profiles[i].reset = usage.reset || profiles[i].reset;
    break;
  }
  renderProfiles();
  if (sideMode === "config" && sideOpen) renderConfig();
}

function refreshOneProfileUsage(profileName) {
  if (!profileName) return Promise.resolve();
  setStatus("Refreshing usage: " + profileName + "...");
  return invokeBackend("refresh_profile_usage", { profileName: profileName })
    .then(function (usage) {
      applyProfileUsage(usage);
      setStatus("Usage refreshed: " + profileName);
      return usage;
    })
    .catch(function (error) {
      console.warn("refresh_profile_usage failed", profileName, error);
      setStatus("Usage refresh skipped: " + profileName);
    });
}

function refreshAllProfileUsage(force) {
  if (isMockMode) return Promise.resolve();
  if (!force && initialUsageRefreshed) return Promise.resolve();
  if (profiles.length === 0) return Promise.resolve();
  initialUsageRefreshed = true;
  setStatus("Refreshing usage for all profiles...");
  var queue = profiles.map(function (profile) {
    return profile.name;
  });

  return new Promise(function (resolve) {
    function next() {
    if (queue.length === 0) {
      setStatus("All profile usage refreshed.");
      resolve();
      return;
    }
    var name = queue.shift();
    refreshOneProfileUsage(name).then(function () {
      setStatus("Usage refresh progress. Remaining profiles: " + queue.length);
      next();
    });
    }

    next();
  });
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

function showDetachedSidePanel(title, bodyHtml) {
  var invoke = getTauriInvoke();
  els.appShell.classList.toggle("panel-config", sideMode === "config");
  els.appShell.classList.toggle("panel-more", sideMode === "more");
  els.closeSideIcon.src = layoutMode === "single" ? "./assets/close-down.png" : "./assets/close.png";
  if (!invoke) {
    els.sideTitle.textContent = title;
    els.sideBody.innerHTML = bodyHtml;
    els.sidePanel.setAttribute("aria-hidden", "false");
    sideOpen = true;
    els.appShell.classList.add("panel-open");
    return;
  }

  invoke("show_side_panel", {
    title: title,
    bodyHtml: bodyHtml,
    baseWidth: getLayoutWindowWidth()
  }).then(function () {
    els.sideTitle.textContent = title;
    els.sideBody.innerHTML = bodyHtml;
    els.sidePanel.setAttribute("aria-hidden", "false");
    sideOpen = true;
    window.requestAnimationFrame(function () {
      els.appShell.classList.add("panel-open");
    });
    setStatus(title + " panel opened.");
  }).catch(function (error) {
    console.warn("show_side_panel failed", error);
    setStatus("Panel failed: " + error);
    showToast("Panel failed");
  });
}

function beginWindowDrag(event) {
  if (event.button !== 0) return;
  var invoke = getTauriInvoke();
  if (!invoke) return;

  event.preventDefault();
  invoke("start_window_drag").catch(function (error) {
    console.warn("start_window_drag failed", error);
  });
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
      "<strong>Auth</strong>" +
      "<pre>" + escapeHtml(profile.authText) + "</pre>" +
      "</section>" +
      '<section class="config-block">' +
      "<strong>Config</strong>" +
      "<pre>" + escapeHtml(profile.configText) + "</pre>" +
      "</section>";
  } else {
    var activeText = configViewMode === "config" ? profile.configText : profile.authText;
    bodyHtml =
      '<section class="config-block single-config-block">' +
      "<pre>" + escapeHtml(activeText) + "</pre>" +
      "</section>";
    els.sideControls.innerHTML =
      '<div class="segment-control ' + (configViewMode === "config" ? "config-active" : "auth-active") + '" role="tablist" aria-label="Config file">' +
      '<button class="' + (configViewMode === "auth" ? "active" : "") + '" data-config-view="auth" role="tab" aria-selected="' + (configViewMode === "auth" ? "true" : "false") + '">Auth</button>' +
      '<button class="' + (configViewMode === "config" ? "active" : "") + '" data-config-view="config" role="tab" aria-selected="' + (configViewMode === "config" ? "true" : "false") + '">Config</button>' +
      "</div>";
  }
  showDetachedSidePanel("Config", bodyHtml);
}

function renderMore() {
  sideMode = "more";
  els.sideControls.innerHTML = "";
  var bodyHtml =
    '<section class="sidebar-actions">' +
    '<button data-more-action="save-current">Save Current as New</button>' +
    '<button data-more-action="renew-usage">Renew Usage</button>' +
    '<button data-more-action="open-app">Open ChatGPT/Codex</button>' +
    '<button data-more-action="open-data">Open Data Dir</button>' +
    '<button data-more-action="open-codex">Open Codex Dir</button>' +
    '<button class="danger-text" data-more-action="hard-kill">Hard Kill ChatGPT/Codex</button>' +
    "</section>";
  showDetachedSidePanel("More", bodyHtml);
}

function switchProfile(name, fromTray) {
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
  });
}

function startLoginWatch() {
  if (loginWatchTimer) {
    window.clearInterval(loginWatchTimer);
  }
  loginStableSignature = "";
  loginStableCount = 0;
  setStatus("Login watch started. Waiting for auth.json...");
  loginWatchTimer = window.setInterval(function () {
    invokeBackend("get_login_state")
      .then(function (state) {
        setStatus(state.message);
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
        window.clearInterval(loginWatchTimer);
        loginWatchTimer = null;
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
        });
      })
      .catch(function (error) {
        setStatus("Login watch failed: " + String(error));
      });
  }, 1000);
}

function loginNewAccount() {
  showConfirmDialog(
    "New Profile Login",
    "Current auth/config will be backed up, then ChatGPT/Codex will restart for login."
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
  }).then(function () {
    showToast("All usage refreshed");
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

document.getElementById("configBtn").onclick = function () {
  renderConfig();
};

document.getElementById("moreBtn").onclick = function () {
  renderMore();
};

document.getElementById("minimizeWindowBtn").onclick = function () {
  invokeWindowCommand("minimize_window");
};

document.getElementById("closeWindowBtn").onclick = function () {
  invokeWindowCommand("close_window");
};

els.windowDragMask.onmousedown = beginWindowDrag;
els.closeSideBtn.onclick = closeSide;
els.sideControls.onclick = function (event) {
  var button = event.target.closest("[data-config-view]");
  if (!button) return;
  configViewMode = button.getAttribute("data-config-view") === "config" ? "config" : "auth";
  renderConfig();
};
els.sideBody.onclick = function (event) {
  var button = event.target.closest("[data-more-action]");
  if (!button) return;
  var action = button.getAttribute("data-more-action");
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
  if (action === "renew-usage") {
    refreshAllProfileUsage(true).then(function () {
      showToast("All usage renewed");
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
els.workspace.onclick = function () {
  if (sideOpen && layoutMode === "single") {
    closeSide();
  }
};

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

listenNativeTray();
listenPanelOpenState();
loadProfiles().catch(function (error) {
  setStatus("Profile load failed: " + String(error));
  renderProfiles();
}).then(function () {
  refreshAllProfileUsage(false);
});
