const API = "http://127.0.0.1:52137";

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
  const url = info.linkUrl || info.pageUrl;
  if (url) sendToDvidm([url]);
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

async function sendToDvidm(urls) {
  const unique = [...new Set(urls.filter((u) => /^https?:\/\//i.test(u)))];
  if (!unique.length) {
    showBadge("0", "#f59e0b");
    return;
  }
  try {
    const res = await fetch(`${API}/add`, {
      method: "POST",
      headers: {
        "Content-Type": "application/json",
        "X-DVIDM": "chrome"
      },
      body: JSON.stringify({ urls: unique })
    });
    if (!res.ok) throw new Error(await res.text());
    showBadge(String(unique.length), "#22c55e");
  } catch (err) {
    showBadge("!", "#ef4444");
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
