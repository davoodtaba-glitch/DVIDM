const API = "http://127.0.0.1:52137";

// Settings published by the app. Content scripts ask the worker instead of
// fetching localhost themselves: Chrome's local-network protection blocks
// page-initiated requests to 127.0.0.1 from public sites.
let config = { formats: [], modifier: "both", at: 0 };
let configFetch = null;

function loadConfig() {
  if (Date.now() - config.at < 2000) return Promise.resolve(config);
  if (configFetch) return configFetch;
  configFetch = fetch(`${API}/formats`, { cache: "no-store" })
    .then((res) => (res.ok ? res.json() : null))
    .then((data) => {
      if (data && Array.isArray(data.formats)) config.formats = data.formats;
      if (data && typeof data.modifier === "string") config.modifier = data.modifier;
      config.at = Date.now();
    })
    .catch(() => {})
    .finally(() => {
      configFetch = null;
    });
  return configFetch.then(() => config);
}

chrome.runtime.onInstalled.addListener(() => {
  chrome.contextMenus.removeAll(() => {
    chrome.contextMenus.create({
      id: "dvidm-link",
      title: "Download link with DVIDM",
      contexts: ["link"]
    });
    chrome.contextMenus.create({
      id: "dvidm-selected",
      title: "Send selected links to DVIDM",
      contexts: ["selection"]
    });
    chrome.contextMenus.create({
      id: "dvidm-copy-selected",
      title: "Copy selected links",
      contexts: ["selection"]
    });
    chrome.contextMenus.create({
      id: "dvidm-page",
      title: "Send this page to DVIDM",
      contexts: ["page"]
    });
  });
});

chrome.contextMenus.onClicked.addListener((info) => {
  if (info.menuItemId === "dvidm-selected") {
    collectForSelection(info);
    return;
  }
  if (info.menuItemId === "dvidm-copy-selected") {
    copySelectedLinks(info);
    return;
  }
  if (info.menuItemId === "dvidm-link" && info.linkUrl) {
    // Feed the link through Chrome's download engine first: that resolves the
    // login redirect and gives the real CDN URL, which a plain /add cannot.
    armTakeover(info.linkUrl, false);
    startMenuFallback(info.linkUrl);
    chrome.downloads.download({ url: info.linkUrl }, () => {
      if (!chrome.runtime.lastError) return;
      cancelMenuFallback();
      resetArm();
      sendToDvidm([info.linkUrl]);
    });
    return;
  }
  const url = info.linkUrl || info.pageUrl;
  if (url) sendToDvidm([url]);
});

// A modified click whose URL does not look like a file can still start a
// browser download (Content-Disposition, scripted buttons). Let Chrome resolve
// the redirect (signed CDN link) and the server filename, then take it over.
const ARM_WINDOW_MS = 8000;
const ARM_WINDOW_NO_HREF_MS = 3000;
let armedAt = 0;
let armedHref = "";
let armedDirect = true;
const MENU_ALARM = "dvidm-menu-fallback";

function cancelMenuFallback() {
  chrome.alarms.clear(MENU_ALARM);
  chrome.storage.session.remove("dvidmMenuUrl");
}

/// The context menu is an explicit "send this to DVIDM". Chrome only reports
/// the download after the server answers, so a dead/slow link would otherwise
/// never reach the app; hand the original URL over after a grace period.
/// Alarms wake the service worker again even if Chrome suspended it.
function startMenuFallback(url) {
  cancelMenuFallback();
  chrome.storage.session.set({ dvidmMenuUrl: url });
  chrome.alarms.create(MENU_ALARM, { when: Date.now() + 6000 });
}

chrome.alarms.onAlarm.addListener((alarm) => {
  if (alarm.name !== MENU_ALARM) return;
  chrome.storage.session.get("dvidmMenuUrl", (data) => {
    const url = data && data.dvidmMenuUrl;
    if (!url) return;
    chrome.storage.session.remove("dvidmMenuUrl");
    resetArm();
    sendToDvidm([url], false);
  });
});

function armTakeover(href, direct) {
  armedAt = Date.now();
  armedHref = href || "";
  armedDirect = direct !== false;
}

function resetArm() {
  armedAt = 0;
  armedHref = "";
  armedDirect = true;
}

function sameTarget(a, b) {
  try {
    const ua = new URL(a);
    const ub = new URL(b);
    return ua.origin === ub.origin && ua.pathname === ub.pathname;
  } catch (err) {
    return a === b;
  }
}

// Ctrl/Alt/Shift/Insert+click on a file link: add it straight to DVIDM.
chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (message && message.type === "dvidm-config") {
    loadConfig().then((c) => sendResponse({ formats: c.formats, modifier: c.modifier }));
    return true;
  }
  if (message && message.type === "dvidm-arm") {
    armTakeover(typeof message.href === "string" ? message.href : "", true);
    return false;
  }
  if (message && message.type === "dvidm-disarm") {
    resetArm();
    return false;
  }
  if (!message || message.type !== "dvidm-direct" || typeof message.url !== "string") {
    return false;
  }
  sendToDvidm([message.url], true).then((ok) => {
    // The click was already consumed; if DVIDM is not running, let Chrome
    // download the file instead of losing it.
    if (!ok) chrome.downloads.download({ url: message.url });
    sendResponse({ ok });
  });
  return true;
});

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

async function dvidmAlive() {
  try {
    const res = await fetch(`${API}/ping`, { cache: "no-store" });
    return res.ok;
  } catch (err) {
    return false;
  }
}

function downloadInfo(id) {
  return new Promise((resolve) => {
    chrome.downloads.search({ id }, (items) => {
      resolve(items && items[0] ? items[0] : null);
    });
  });
}

function cancelDownload(id) {
  return new Promise((resolve) => {
    chrome.downloads.cancel(id, () => {
      chrome.downloads.erase({ id }, () => resolve());
    });
  });
}

async function takeOverDownload(id, direct = true) {
  if (!(await dvidmAlive())) return;
  let info = null;
  for (let attempt = 0; attempt < 12; attempt++) {
    info = await downloadInfo(id);
    if (!info) return;
    if (info.finalUrl && info.filename) break;
    await sleep(150);
  }
  if (!info) return;
  const url = info.finalUrl || info.url;
  if (!/^https?:\/\//i.test(url)) return;
  // The app may have closed while Chrome was resolving the redirect.
  if (!(await dvidmAlive())) return;
  const name = info.filename ? info.filename.split(/[\\/]/).pop() : "";
  await cancelDownload(id);
  const ok = await sendToDvidm([url], direct, name);
  if (!ok) chrome.downloads.download({ url, filename: name || undefined });
}

function armedNow(item) {
  if (!armedAt) return false;
  const limit = armedHref ? ARM_WINDOW_MS : ARM_WINDOW_NO_HREF_MS;
  if (Date.now() - armedAt > limit) return false;
  if (!armedHref) return true;
  if (sameTarget(item.url || "", armedHref)) return true;
  return Boolean(item.finalUrl && sameTarget(item.finalUrl, armedHref));
}

function takeArmed(item) {
  const direct = armedDirect;
  cancelMenuFallback();
  resetArm();
  void takeOverDownload(item.id, direct);
}

chrome.downloads.onCreated.addListener((item) => {
  if (armedNow(item)) {
    takeArmed(item);
    return;
  }
  // The arm message from mousedown may still be in flight while the service
  // worker wakes up; give it a moment before letting Chrome keep the file.
  setTimeout(() => {
    if (armedNow(item)) takeArmed(item);
  }, 400);
});

function collectForSelection(info) {
  collectSelectedUrls(info, (urls, error) => {
    if (urls.length) sendToDvidm(urls);
    else showBadge(error ? "!" : "0", error ? "#ef4444" : "#f59e0b");
  });
}

function copySelectedLinks(info) {
  collectSelectedUrls(info, async (urls, error) => {
    if (!urls.length) {
      showBadge(error ? "!" : "0", error ? "#ef4444" : "#f59e0b");
      return;
    }
    try {
      const copied = await copyToClipboard(urls.join("\n"));
      showBadge(copied ? String(urls.length) : "!", copied ? "#3b82f6" : "#ef4444");
    } catch (err) {
      showBadge("!", "#ef4444");
    }
  });
}

// Use the same path as the popup button, which is known to work: find the
// active tab and inject the collector there.
function collectSelectedUrls(info, callback) {
  const textUrls = extractUrls(info.selectionText || "");
  chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    const tab = tabs[0];
    if (!tab || tab.id === undefined) {
      callback(textUrls, false);
      return;
    }
    chrome.scripting.executeScript(
      { target: { tabId: tab.id, allFrames: true }, func: collectSelectedLinks },
      (results) => {
        const error = chrome.runtime.lastError;
        const found = [];
        for (const frame of results || []) {
          if (Array.isArray(frame.result)) found.push(...frame.result);
        }
        callback([...new Set([...textUrls, ...found])], Boolean(error));
      }
    );
  });
}

async function ensureOffscreenDocument() {
  const url = chrome.runtime.getURL("offscreen.html");
  if (chrome.runtime.getContexts) {
    const contexts = await chrome.runtime.getContexts({
      contextTypes: ["OFFSCREEN_DOCUMENT"],
      documentUrls: [url]
    });
    if (contexts.length) return;
  }
  try {
    await chrome.offscreen.createDocument({
      url,
      reasons: ["CLIPBOARD"],
      justification: "Copy selected links to the clipboard"
    });
  } catch (err) {
    // A concurrent call may have created it already.
  }
}

async function copyToClipboard(text) {
  await ensureOffscreenDocument();
  // The offscreen script can still be loading right after creation.
  for (let attempt = 0; attempt < 3; attempt++) {
    try {
      const response = await chrome.runtime.sendMessage({ type: "dvidm-copy", text });
      if (response) return Boolean(response.ok);
    } catch (err) {
      // no receiver yet
    }
    await new Promise((resolve) => setTimeout(resolve, 100));
  }
  return false;
}

async function sendToDvidm(urls, direct = false, fileName = "") {
  const unique = [...new Set(urls.filter((u) => /^https?:\/\//i.test(u)))];
  if (!unique.length) {
    showBadge("0", "#f59e0b");
    return false;
  }
  try {
    const res = await fetch(`${API}/add`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-DVIDM": "chrome"
      },
      body: JSON.stringify({ urls: unique, direct, file_name: fileName || null })
    });
    if (!res.ok) throw new Error(await res.text());
    showBadge(String(unique.length), "#22c55e");
    return true;
  } catch (err) {
    showBadge("!", "#ef4444");
    return false;
  }
}

function showBadge(text, color) {
  chrome.action.setBadgeBackgroundColor({ color });
  chrome.action.setBadgeText({ text });
  setTimeout(() => chrome.action.setBadgeText({ text: "" }), 3000);
}

function extractUrls(text) {
  const urls = [];
  const seen = new Set();
  const re = /https?:\/\/[^\s"'<>]+/gi;
  let match;
  while ((match = re.exec(text))) {
    const url = match[0].replace(/[.,;:)\]}>]+$/, "");
    if (!seen.has(url)) {
      seen.add(url);
      urls.push(url);
    }
  }
  return urls;
}

// Runs inside the page via chrome.scripting.executeScript.
function collectSelectedLinks() {
  const selection = window.getSelection();
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
    } catch (e) {
      // ignore malformed URLs
    }
  };
  const rangeCount = selection ? selection.rangeCount : 0;
  for (let i = 0; i < rangeCount; i++) {
    const range = selection.getRangeAt(i);
    const fragment = range.cloneContents();
    const container = document.createElement("div");
    container.appendChild(fragment);
    container.querySelectorAll("a[href]").forEach((a) => add(a.getAttribute("href")));
    const text = range.toString();
    const re = /https?:\/\/[^\s"'<>]+/gi;
    let match;
    while ((match = re.exec(text))) {
      add(match[0].replace(/[.,;:)\]}>]+$/, ""));
    }
  }
  return urls;
}
