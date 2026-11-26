(function () {
  var invoke = window.__TAURI__ && window.__TAURI__.core && window.__TAURI__.core.invoke;
  window.codexSwitchPlatform = { os: "unknown", arch: "unknown", nativeQrCapture: false };
  window.codexSwitchPlatformReady = invoke ? invoke("get_platform_info").then(function (info) {
    window.codexSwitchPlatform = info;
    document.documentElement.dataset.platform = info.os;
    return info;
  }).catch(function (error) {
    console.warn("Platform information unavailable", error);
    return window.codexSwitchPlatform;
  }) : Promise.resolve(window.codexSwitchPlatform);
})();
