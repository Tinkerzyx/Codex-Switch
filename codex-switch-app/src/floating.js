var floatingTitle = document.getElementById("floatingTitle");
var floatingUsage = document.getElementById("floatingUsage");
var floatingBar = document.getElementById("floatingBar");

function applyFloatingPayload(payload) {
  payload = payload || {};
  floatingTitle.textContent = payload.title || "Codex Switch";
  floatingUsage.textContent = payload.usage || "-";
}

if (window.__TAURI__ && window.__TAURI__.event && window.__TAURI__.event.listen) {
  window.__TAURI__.event.listen("floating-bar-content", function (event) {
    applyFloatingPayload(event.payload);
  });
}

if (window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke) {
  window.__TAURI__.core.invoke("get_floating_bar").then(applyFloatingPayload).catch(function (error) {
    console.warn("get_floating_bar failed", error);
  });
}

floatingBar.ondblclick = function () {
  if (!window.__TAURI__ || !window.__TAURI__.core || !window.__TAURI__.core.invoke) return;
  window.__TAURI__.core.invoke("restore_main_window").catch(function (error) {
    console.warn("restore_main_window failed", error);
  });
};

floatingBar.addEventListener("pointerdown", function (event) {
  if (event.button !== 0 || event.detail > 1) return;
  if (window.__TAURI__ && window.__TAURI__.window) {
    window.__TAURI__.window.getCurrentWindow().startDragging().catch(function (error) {
      console.warn("Floating bar drag failed", error);
    });
  }
});

document.addEventListener("contextmenu", function (event) {
  event.preventDefault();
});
