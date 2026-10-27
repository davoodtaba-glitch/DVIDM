// Click a file link with the configured modifier (Ctrl/Alt) to download with DVIDM.
// Only links that look like files are captured, so Ctrl+click on an ordinary
// page link keeps opening a new tab. The captured file types come from
// DVIDM's settings (Settings → Browser shortcut); the built-in defaults apply
// until the app answers, so the shortcut works even before the first fetch.
(() => {
  const DEFAULT_FILE_RE =
    /\.(?:7z|apk|avi|bin|bz2|cab|deb|dmg|docx?|epub|exe|flac|gz|img|iso|jpe?g|m4a|m4v|mkv|mov|mp3|mp4|msi|msix|msp|odt|ogg|otf|pdf|pkg|png|ppt[xm]?|rar|rpm|tar|tgz|torrent|ttf|wav|webm|wim|xlsx?|xz|zip)$/i;
  const MODIFIERS = ["ctrl", "alt", "shift", "insert", "ctrl_alt", "ctrl_shift", "alt_shift", "both"];

  let fileRe = DEFAULT_FILE_RE;
  let modifier = "both";
  let insertHeld = false;

  window.addEventListener("keydown", (event) => {
    if (event.key === "Insert") insertHeld = true;
  }, true);
  window.addEventListener("keyup", (event) => {
    if (event.key === "Insert") insertHeld = false;
  }, true);
  window.addEventListener("blur", () => {
    insertHeld = false;
  }, true);
  document.addEventListener("visibilitychange", () => {
    if (document.hidden) insertHeld = false;
  }, true);

  function modifierMatches(event) {
    const ctrl = event.ctrlKey;
    const alt = event.altKey;
    const shift = event.shiftKey;
    switch (modifier) {
      case "ctrl":
        return ctrl && !alt && !shift;
      case "alt":
        return alt && !ctrl && !shift;
      case "shift":
        return shift && !ctrl && !alt;
      case "insert":
        return insertHeld && !ctrl && !alt && !shift;
      case "ctrl_alt":
        return ctrl && alt && !shift;
      case "ctrl_shift":
        return ctrl && shift && !alt;
      case "alt_shift":
        return alt && shift && !ctrl;
      default:
        // "both": Ctrl or Alt; leave Chrome's Ctrl+Shift new-tab gesture alone.
        if (!ctrl && !alt) return false;
        return !(ctrl && shift);
    }
  }

  function anchorHref(event) {
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
    return anchor ? anchor.href : "";
  }

  // The clicked URL may not end in a known extension (Content-Disposition
  // downloads, scripted buttons). The background cancels a download that
  // starts right after a modified click and hands it to DVIDM instead.
  function armDownloadTakeover(href) {
    try {
      chrome.runtime.sendMessage(
        { type: "dvidm-arm", href: href || "" },
        () => void chrome.runtime.lastError
      );
    } catch (err) {
      // The extension was reloaded; the page needs a refresh.
    }
  }

  function disarmDownloadTakeover() {
    try {
      chrome.runtime.sendMessage({ type: "dvidm-disarm" }, () => void chrome.runtime.lastError);
    } catch (err) {
      // The extension was reloaded; the page needs a refresh.
    }
  }

  // Arm on mousedown so the message reaches the service worker before Chrome
  // starts the download on mouseup/click.
  window.addEventListener("mousedown", (event) => {
    if (event.button === 0 && modifierMatches(event)) {
      armDownloadTakeover(anchorHref(event));
    }
  }, true);

  function buildRe(exts) {
    const clean = [...new Set(
      exts
        .map((ext) => String(ext).trim().replace(/^\.+/, "").toLowerCase())
        .filter((ext) => /^[a-z0-9]{1,32}$/.test(ext))
    )];
    if (!clean.length) return DEFAULT_FILE_RE;
    try {
      return new RegExp("\\.(?:" + clean.join("|") + ")$", "i");
    } catch (err) {
      return DEFAULT_FILE_RE;
    }
  }

  // The service worker holds the latest settings; asking it avoids page-side
  // requests to localhost, which Chrome's local-network protection can block.
  function refreshFormats() {
    try {
      chrome.runtime.sendMessage({ type: "dvidm-config" }, (data) => {
        void chrome.runtime.lastError;
        if (!data) return;
        if (Array.isArray(data.formats)) fileRe = buildRe(data.formats);
        if (MODIFIERS.includes(data.modifier)) {
          modifier = data.modifier;
        }
      });
    } catch (err) {
      // The extension was reloaded; the page needs a refresh.
    }
  }
  refreshFormats();
  setInterval(refreshFormats, 5000);

  function fileLink(event) {
    if (event.button !== 0 || event.defaultPrevented) return null;
    if (!modifierMatches(event)) return null;

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
    return fileRe.test(pathname) ? href : null;
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
      // The direct path prevents Chrome's download; no takeover needed.
      disarmDownloadTakeover();
      // The shortcut is ours: stop Chrome from opening or downloading the
      // link itself.
      event.preventDefault();
      event.stopImmediatePropagation();
      try {
        chrome.runtime.sendMessage({ type: "dvidm-direct", url: href }, (response) => {
          const failed = chrome.runtime.lastError || !response || !response.ok;
          hint(
            failed
              ? "DVIDM is not running - Chrome is downloading the file"
              : "Download added to DVIDM",
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
