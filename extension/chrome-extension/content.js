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
  // starts the download on mouseup/click. Clicks inside our selection menu are
  // not page actions.
  window.addEventListener("mousedown", (event) => {
    if (event.target && event.target.closest && event.target.closest("#dvidm-selection-menu")) {
      return;
    }
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

  // Floating menu next to the pointer with the same actions as the right-click
  // menu. A page overlay is the only way to show this; Chrome does not let an
  // extension extend the native context menu inline.
  let selectionMenu = null;
  let selectionMenuPoint = null;
  let selectionMenuEnabled = true;

  try {
    chrome.storage.local.get({ dvidmSelectionMenu: true }, (data) => {
      selectionMenuEnabled = data.dvidmSelectionMenu !== false;
    });
    chrome.storage.onChanged.addListener((changes, area) => {
      if (area !== "local" || !changes.dvidmSelectionMenu) return;
      selectionMenuEnabled = changes.dvidmSelectionMenu.newValue !== false;
      if (!selectionMenuEnabled) closeSelectionMenu();
    });
  } catch (err) {
    // Storage is optional; keep the menu enabled.
  }

  function closeSelectionMenu() {
    const menu = selectionMenu;
    selectionMenu = null;
    if (menu && menu.parentNode) {
      menu.remove();
    }
  }

  function selectionAnchor(selection) {
    const node = selection.anchorNode;
    const element = node && (node.nodeType === 1 ? node : node.parentElement);
    const anchor = element && element.closest ? element.closest("a[href]") : null;
    return anchor && /^https?:/i.test(anchor.href) ? anchor.href : "";
  }

  function selectionLinks(selection) {
    const urls = [];
    const seen = new Set();
    const add = (value) => {
      if (!value) return;
      try {
        const absolute = new URL(value, location.href).href;
        if (/^https?:/i.test(absolute) && !seen.has(absolute)) {
          seen.add(absolute);
          urls.push(absolute);
        }
      } catch (err) {
        // ignore malformed URLs
      }
    };
    // A selection that starts inside a link may not clone the anchor itself.
    const anchor = selectionAnchor(selection);
    if (anchor) add(anchor);
    for (let i = 0; i < selection.rangeCount; i++) {
      const range = selection.getRangeAt(i);
      const fragment = range.cloneContents();
      const container = document.createElement("div");
      container.appendChild(fragment);
      container.querySelectorAll("a[href]").forEach((a) => add(a.getAttribute("href")));
      const re = /https?:\/\/[^\s"'<>]+/gi;
      let match;
      while ((match = re.exec(range.toString()))) {
        add(match[0].replace(/[.,;:)\]}>]+$/, ""));
      }
    }
    return urls;
  }

  function menuIcon(paths, color) {
    const svg = document.createElementNS("http://www.w3.org/2000/svg", "svg");
    svg.setAttribute("viewBox", "0 0 24 24");
    svg.setAttribute("width", "16");
    svg.setAttribute("height", "16");
    svg.setAttribute("fill", "none");
    svg.setAttribute("stroke", color);
    svg.setAttribute("stroke-width", "1.8");
    svg.setAttribute("stroke-linecap", "round");
    svg.setAttribute("stroke-linejoin", "round");
    svg.style.flexShrink = "0";
    for (const d of paths) {
      const path = document.createElementNS("http://www.w3.org/2000/svg", "path");
      path.setAttribute("d", d);
      svg.appendChild(path);
    }
    return svg;
  }

  function menuButton(iconPaths, label, enabled, primary, onClick) {
    const button = document.createElement("button");
    button.type = "button";
    button.style.cssText = [
      "display:flex",
      "align-items:center",
      "gap:9px",
      "width:100%",
      "box-sizing:border-box",
      "margin:0",
      "padding:8px 10px",
      "border:0",
      "border-radius:7px",
      "background:" + (primary ? "rgba(18,236,145,0.08)" : "transparent"),
      "color:#eef1f6",
      "font:600 13px/1.3 'Segoe UI',system-ui,sans-serif",
      "text-align:left",
      "cursor:pointer",
      "white-space:nowrap",
      "transition:background .12s ease"
    ].join(";");
    button.appendChild(menuIcon(iconPaths, primary ? "#12ec91" : "#9aa3b5"));
    const text = document.createElement("span");
    text.textContent = label;
    button.appendChild(text);
    if (!enabled) {
      button.disabled = true;
      button.style.opacity = "0.42";
      button.style.cursor = "default";
      button.style.background = "transparent";
      return button;
    }
    button.addEventListener("mouseenter", () => {
      button.style.background = primary ? "rgba(18,236,145,0.13)" : "#1b202a";
    });
    button.addEventListener("mouseleave", () => {
      button.style.background = primary ? "rgba(18,236,145,0.08)" : "transparent";
    });
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      onClick();
    });
    return button;
  }

  function menuSeparator() {
    const line = document.createElement("div");
    line.style.cssText = "height:1px;background:#232833;margin:4px 8px";
    return line;
  }

  function showSelectionMenu() {
    if (!selectionMenuEnabled) {
      closeSelectionMenu();
      return;
    }
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed || selection.rangeCount === 0) {
      closeSelectionMenu();
      return;
    }
    if (selection.toString().trim().length < 2) {
      closeSelectionMenu();
      return;
    }
    const node = selection.anchorNode;
    const element = node && (node.nodeType === 1 ? node : node.parentElement);
    if (element && element.closest && element.closest("input, textarea, [contenteditable='true']")) {
      closeSelectionMenu();
      return;
    }

    const urls = selectionLinks(selection);
    const anchor = selectionAnchor(selection);
    const count = urls.length;

    const send = () => {
      closeSelectionMenu();
      try {
        chrome.runtime.sendMessage(
          { type: "dvidm-send-urls", urls: urls, direct: false },
          (response) => {
            void chrome.runtime.lastError;
            const ok = Boolean(response && response.ok);
            hint(ok ? "Sent to DVIDM" : "DVIDM is not running", ok);
          }
        );
      } catch (err) {
        hint("DVIDM is not running", false);
      }
    };
    const copy = () => {
      closeSelectionMenu();
      try {
        chrome.runtime.sendMessage({ type: "dvidm-copy-urls", urls: urls }, (response) => {
          void chrome.runtime.lastError;
          const ok = Boolean(response && response.ok);
          hint(ok ? "Links copied" : "Could not copy links", ok);
        });
      } catch (err) {
        hint("Could not copy links", false);
      }
    };
    const downloadLink = () => {
      closeSelectionMenu();
      try {
        chrome.runtime.sendMessage({ type: "dvidm-download-link", url: anchor }, () => {
          void chrome.runtime.lastError;
        });
        hint("Resolving link in Chrome…", true);
      } catch (err) {
        hint("DVIDM is not running", false);
      }
    };
    const sendPage = () => {
      closeSelectionMenu();
      try {
        chrome.runtime.sendMessage(
          { type: "dvidm-send-urls", urls: [location.href], direct: false },
          (response) => {
            void chrome.runtime.lastError;
            const ok = Boolean(response && response.ok);
            hint(ok ? "Sent to DVIDM" : "DVIDM is not running", ok);
          }
        );
      } catch (err) {
        hint("DVIDM is not running", false);
      }
    };

    closeSelectionMenu();
    const menu = document.createElement("div");
    menu.id = "dvidm-selection-menu";
    menu.style.cssText = [
      "position:fixed",
      "z-index:2147483647",
      "min-width:268px",
      "max-width:324px",
      "padding:6px",
      "background:#101318",
      "color:#eef1f6",
      "border:1px solid #2b3140",
      "border-radius:11px",
      "box-shadow:0 18px 45px rgba(0,0,0,.55),0 2px 8px rgba(0,0,0,.4)",
      "font:13px/1.3 'Segoe UI',system-ui,sans-serif",
      "direction:ltr",
      "text-align:left"
    ].join(";");

    const header = document.createElement("div");
    header.style.cssText =
      "display:flex;align-items:center;gap:9px;padding:6px 5px 8px 9px;margin-bottom:4px;border-bottom:1px solid #232833";
    const mark = document.createElement("div");
    mark.textContent = "DV";
    mark.style.cssText =
      "display:flex;align-items:center;justify-content:center;width:22px;height:22px;border-radius:6px;background:#12ec91;color:#06130c;font:800 10px/1 'Segoe UI',sans-serif";
    const titles = document.createElement("div");
    titles.style.cssText = "display:flex;flex-direction:column;gap:1px;min-width:0";
    const title = document.createElement("div");
    title.textContent = "DVIDM";
    title.style.cssText =
      "font:700 12.5px/1.2 'Segoe UI',sans-serif;letter-spacing:.4px;color:#f4f7fb";
    const subtitle = document.createElement("div");
    subtitle.textContent = count
      ? count + " link" + (count === 1 ? "" : "s") + " selected"
      : "No links selected";
    subtitle.style.cssText =
      "font:500 10px/1.2 'Segoe UI',sans-serif;color:#8a93a6";
    titles.appendChild(title);
    titles.appendChild(subtitle);
    header.appendChild(mark);
    header.appendChild(titles);
    const closeButton = document.createElement("button");
    closeButton.type = "button";
    closeButton.title = "Close";
    closeButton.setAttribute("aria-label", "Close");
    closeButton.style.cssText = [
      "display:flex",
      "align-items:center",
      "justify-content:center",
      "width:22px",
      "height:22px",
      "margin-left:auto",
      "padding:0",
      "border:0",
      "border-radius:6px",
      "background:transparent",
      "color:#8a93a6",
      "cursor:pointer",
      "flex-shrink:0"
    ].join(";");
    closeButton.appendChild(menuIcon(["M6 6l12 12", "M18 6L6 18"], "#8a93a6"));
    closeButton.addEventListener("mouseenter", () => {
      closeButton.style.background = "#1b202a";
      const icon = closeButton.querySelector("svg");
      if (icon) icon.setAttribute("stroke", "#eef1f6");
    });
    closeButton.addEventListener("mouseleave", () => {
      closeButton.style.background = "transparent";
      const icon = closeButton.querySelector("svg");
      if (icon) icon.setAttribute("stroke", "#8a93a6");
    });
    closeButton.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      closeSelectionMenu();
    });
    header.appendChild(closeButton);
    menu.appendChild(header);

    menu.appendChild(
      menuButton(
        ["M7 17L17 7", "M8 7h9v9"],
        count
          ? "Send " + count + " link" + (count === 1 ? "" : "s") + " to DVIDM"
          : "Send selected links to DVIDM",
        count > 0,
        true,
        send
      )
    );
    menu.appendChild(
      menuButton(
        ["M9 9h9v9H9z", "M15 9V6a1 1 0 0 0-1-1H6a1 1 0 0 0-1 1v8a1 1 0 0 0 1 1h3"],
        count
          ? "Copy " + count + " link" + (count === 1 ? "" : "s")
          : "Copy selected links",
        count > 0,
        false,
        copy
      )
    );
    if (anchor) {
      menu.appendChild(
        menuButton(
          ["M12 4v11", "M7 10l5 5 5-5", "M5 20h14"],
          "Download this link with DVIDM",
          true,
          false,
          downloadLink
        )
      );
    }
    menu.appendChild(menuSeparator());
    menu.appendChild(
      menuButton(
        ["M12 3a9 9 0 1 0 0 18a9 9 0 0 0 0-18", "M3 12h18", "M12 3c2.8 3 2.8 15 0 18", "M12 3c-2.8 3-2.8 15 0 18"],
        "Send this page to DVIDM",
        true,
        false,
        sendPage
      )
    );

    const footer = document.createElement("label");
    footer.style.cssText =
      "display:flex;align-items:center;gap:7px;padding:8px 9px 5px;margin-top:4px;border-top:1px solid #232833;color:#8a93a6;font:500 10.5px/1.2 'Segoe UI',sans-serif;cursor:pointer;user-select:none";
    const check = document.createElement("input");
    check.type = "checkbox";
    check.checked = selectionMenuEnabled;
    check.style.cssText = "width:13px;height:13px;margin:0;accent-color:#12ec91;cursor:pointer";
    check.addEventListener("change", () => {
      selectionMenuEnabled = check.checked;
      try {
        chrome.storage.local.set({ dvidmSelectionMenu: selectionMenuEnabled });
      } catch (err) {
        // Storage is optional.
      }
      if (!selectionMenuEnabled) closeSelectionMenu();
    });
    const footerText = document.createElement("span");
    footerText.textContent = "Show on text selection";
    footer.appendChild(check);
    footer.appendChild(footerText);
    footer.addEventListener("click", (event) => event.stopPropagation());
    menu.appendChild(footer);

    menu.addEventListener("mousedown", (event) => event.stopPropagation());
    menu.addEventListener("click", (event) => event.stopPropagation());
    document.documentElement.appendChild(menu);
    selectionMenu = menu;

    const width = menu.offsetWidth || 280;
    const height = menu.offsetHeight || 210;
    let left;
    let top;
    if (selectionMenuPoint) {
      left = selectionMenuPoint.x + 12;
      top = selectionMenuPoint.y + 12;
      if (left + width > window.innerWidth - 8) {
        left = selectionMenuPoint.x - width - 12;
      }
      if (top + height > window.innerHeight - 8) {
        top = selectionMenuPoint.y - height - 12;
      }
    } else {
      const rect = selection.getRangeAt(0).getBoundingClientRect();
      left = rect.left;
      top = rect.bottom + 8;
      if (top + height > window.innerHeight - 8) {
        top = rect.top - height - 8;
      }
    }
    left = Math.max(8, Math.min(left, window.innerWidth - width - 8));
    top = Math.max(8, Math.min(top, window.innerHeight - height - 8));
    menu.style.left = left + "px";
    menu.style.top = top + "px";
  }

  let selectionMenuTimer = 0;
  function scheduleSelectionMenu() {
    clearTimeout(selectionMenuTimer);
    selectionMenuTimer = setTimeout(showSelectionMenu, 25);
  }

  document.addEventListener("mouseup", (event) => {
    if (event.button !== 0) return;
    if (selectionMenu && selectionMenu.contains(event.target)) return;
    selectionMenuPoint = { x: event.clientX, y: event.clientY };
    scheduleSelectionMenu();
  }, true);
  document.addEventListener("keyup", (event) => {
    if (event.key === "Escape") {
      closeSelectionMenu();
      return;
    }
    if (
      event.shiftKey ||
      ["ArrowLeft", "ArrowRight", "ArrowUp", "ArrowDown", "Home", "End"].includes(event.key)
    ) {
      selectionMenuPoint = null;
      scheduleSelectionMenu();
      return;
    }
    const selection = window.getSelection();
    if (!selection || selection.isCollapsed) closeSelectionMenu();
  }, true);
  window.addEventListener("scroll", closeSelectionMenu, true);
  window.addEventListener("resize", closeSelectionMenu, true);
  window.addEventListener("blur", closeSelectionMenu, true);
  window.addEventListener("mousedown", (event) => {
    if (selectionMenu && !selectionMenu.contains(event.target)) closeSelectionMenu();
  }, true);

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
