// Runs at document start, only in the isolated account-login window.
(function () {
  if (location.hostname !== "chatgpt.com") return;

  var overlay = document.createElement("div");
  overlay.style.cssText = "position:fixed!important;inset:0!important;z-index:2147483647!important;display:block!important;visibility:visible!important;";
  var shadow = overlay.attachShadow({ mode: "closed" });
  shadow.innerHTML = '<style>:host{all:initial}.screen{position:fixed;inset:0;display:flex;align-items:center;justify-content:center;background:#f7f8fa;color:#202735;font:16px system-ui,"Microsoft YaHei",sans-serif}.card{text-align:center;padding:32px}.spinner{width:32px;height:32px;margin:0 auto 24px;border:3px solid #dde3ed;border-top-color:#526ee8;border-radius:50%;animation:spin .9s linear infinite}h1{font-size:21px;font-weight:600;margin:0 0 12px}p{font-size:14px;color:#697386;margin:0;line-height:1.7}@keyframes spin{to{transform:rotate(360deg)}}@media(prefers-reduced-motion:reduce){.spinner{animation:none}}</style><div class="screen" role="status" aria-live="polite"><div class="card"><div class="spinner" aria-hidden="true"></div><h1>正在转到账号授权页面…</h1><p>登录完成后将自动继续，请稍候。<br>Preparing Codex authorization…</p></div></div>';
  var style = document.createElement("style");
  style.textContent = 'html[data-csw-login-transition] body{visibility:hidden!important;pointer-events:none!important}';
  var visible = false;
  var confirmed = false;
  var inertBody = null;
  var previousInert = false;
  var lastPath = "";

  function isChatPage() {
    // Keep sign-in, callback and challenge pages available for user interaction.
    return location.pathname === "/" || /^\/(c|g)(\/|$)/.test(location.pathname);
  }

  function render() {
    var root = document.documentElement;
    if (!root) return;
    if (!isChatPage()) visible = false;
    if (visible) {
      root.setAttribute("data-csw-login-transition", "");
      if (!style.isConnected) root.appendChild(style);
      if (!overlay.isConnected) root.appendChild(overlay);
      if (document.body && inertBody !== document.body) {
        if (inertBody) inertBody.inert = previousInert;
        inertBody = document.body;
        previousInert = inertBody.inert;
        inertBody.inert = true;
      }
    } else {
      root.removeAttribute("data-csw-login-transition");
      overlay.remove();
      style.remove();
      if (inertBody) inertBody.inert = previousInert;
      inertBody = null;
    }
  }

  window.__cswShowLoginTransition = function () {
    confirmed = true;
    visible = isChatPage();
    render();
  };

  async function checkPage() {
    var path = location.pathname;
    if (path === lastPath) return render();
    lastPath = path;
    visible = isChatPage();
    render();
    if (!visible || confirmed) return;
    // Mask the first chat-page paint; release it if this is a signed-out page
    // or the session check fails, so login controls can never stay covered.
    var controller = new AbortController();
    var timeout = setTimeout(function () { controller.abort(); }, 8000);
    try {
      var response = await fetch("https://chatgpt.com/api/auth/session", {
        credentials: "include", cache: "no-store",
        headers: { accept: "application/json" }, signal: controller.signal
      });
      var session = response.ok ? await response.json() : null;
      if (!confirmed && location.pathname === path) {
        visible = !!(session && session.accessToken && session.account && session.account.id);
        render();
      }
    } catch (_) {
      if (!confirmed && location.pathname === path) {
        visible = false;
        render();
      }
    } finally {
      clearTimeout(timeout);
    }
  }

  new MutationObserver(function () { checkPage(); }).observe(document, { childList: true, subtree: true });
  ["pushState", "replaceState"].forEach(function (name) {
    var original = history[name];
    history[name] = function () {
      var result = original.apply(this, arguments);
      checkPage();
      return result;
    };
  });
  window.addEventListener("popstate", checkPage);
  checkPage();
})();
