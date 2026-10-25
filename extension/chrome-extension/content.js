// Ctrl+click or Alt+click a file link to download it with DVIDM.
// Only links that look like files are captured, so Ctrl+click on an ordinary
// page link keeps opening a new tab.
(() => {
  const FILE_RE =
    /\.(?:7z|apk|avi|bin|bz2|cab|deb|dmg|docx?|epub|exe|flac|gz|img|iso|jpe?g|m4a|m4v|mkv|mov|mp3|mp4|msi|msix|msp|odt|ogg|otf|pdf|pkg|png|ppt[xm]?|rar|rpm|tar|tgz|torrent|ttf|wav|webm|wim|xlsx?|xz|zip)$/i;

  function fileLink(event) {
    if (event.button !== 0 || event.defaultPrevented) return null;
    if (!event.ctrlKey && !event.altKey) return null;
    // Ctrl+Shift+click is Chrome's "new tab and switch" gesture; leave it.
    if (event.ctrlKey && event.shiftKey) return null;

    const path = typeof event.composedPath === "function" ? event.composedPath() : [];
    let anchor = null;
    for (const node of path) {
      if (node && node.tagName === "A") {
        anchor = node;
        break;
      }
    }
    if (!anchor && event.target && event.target.closest) {
      anchor = event.target.closest("a[href]");
    }
    if (!anchor) return null;

    const href = anchor.href;
    if (!/^https?:/i.test(href)) return null;
    if (anchor.hasAttribute("download")) return href;

    let pathname = "";
    try {
      pathname = new URL(href).pathname;
    } catch (err) {
      return null;
    }
    return FILE_RE.test(pathname) ? href : null;
  }

  function hint(text, ok) {
    if (!document.documentElement) return;
    let el = document.getElementById("dvidm-page-hint");
    if (!el) {
      el = document.createElement("div");
      el.id = "dvidm-page-hint";
      el.style.cssText = [
        "position:fixed",
        "right:18px",
        "bottom:18px",
        "z-index:2147483647",
        "pointer-events:none",
        "font:13px/1.4 'Segoe UI',system-ui,sans-serif",
        "color:#f0f2f8",
        "background:#14161d",
        "border:1px solid #2b3040",
        "border-left:3px solid #12ec91",
        "border-radius:8px",
        "padding:9px 13px",
        "box-shadow:0 10px 28px rgba(0,0,0,.45)",
        "opacity:0",
        "transition:opacity .15s ease"
      ].join(";");
      document.documentElement.appendChild(el);
    }
    el.style.borderLeftColor = ok ? "#12ec91" : "#e65858";
    el.textContent = text;
    el.style.opacity = "1";
    clearTimeout(el._dvidmTimer);
    el._dvidmTimer = setTimeout(() => {
      el.style.opacity = "0";
    }, 2200);
  }

  window.addEventListener(
    "click",
    (event) => {
      const href = fileLink(event);
      if (!href) return;
      // The shortcut is ours: stop Chrome from opening or downloading the
      // link itself.
      event.preventDefault();
      event.stopImmediatePropagation();
      try {
        chrome.runtime.sendMessage({ type: "dvidm-direct", url: href }, (response) => {
          const failed = chrome.runtime.lastError || !response || !response.ok;
          hint(
            failed ? "DVIDM is not running - start it and try again" : "Download added to DVIDM",
            !failed
          );
        });
      } catch (err) {
        hint("DVIDM is not running - start it and try again", false);
      }
    },
    true
  );
})();
