const API = "http://127.0.0.1:52137";
const sendButton = document.getElementById("send");
const copyButton = document.getElementById("copy");
const statusEl = document.getElementById("status");
const SEND_LABEL = "Send selected links on page";
const COPY_LABEL = "Copy selected links";
const resetTimers = new Map();

async function ping() {
  try {
    const res = await fetch(`${API}/ping`, { cache: "no-store" });
    return res.ok;
  } catch {
    return false;
  }
}

function setStatus(ok) {
  statusEl.textContent = ok ? "Connected" : "Offline";
  statusEl.className = "pill " + (ok ? "ok" : "bad");
}

function showLabel(button, text, resetAfter, defaultLabel) {
  button.textContent = text;
  clearTimeout(resetTimers.get(button));
  if (resetAfter) {
    resetTimers.set(
      button,
      setTimeout(() => {
        button.textContent = defaultLabel;
      }, resetAfter)
    );
  }
}

function finish(button, text, resetAfter, defaultLabel) {
  button.disabled = false;
  showLabel(button, text, resetAfter, defaultLabel);
}

async function postUrls(urls) {
  const res = await fetch(`${API}/add`, {
    method: "POST",
    headers: {
      "Content-Type": "application/json",
      "X-DVIDM": "chrome"
    },
    body: JSON.stringify({ urls })
  });
  if (!res.ok) throw new Error(await res.text());
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    const area = document.createElement("textarea");
    area.value = text;
    area.style.position = "fixed";
    area.style.opacity = "0";
    document.body.appendChild(area);
    area.select();
    let ok = false;
    try {
      ok = document.execCommand("copy");
    } catch {
      // ignore, report failure below
    }
    area.remove();
    return ok;
  }
}

function collectSelectedLinks(callback) {
  chrome.tabs.query({ active: true, currentWindow: true }, (tabs) => {
    const tab = tabs[0];
    if (!tab || tab.id === undefined) {
      callback({ error: "No active tab" });
      return;
    }
    chrome.scripting.executeScript(
      { target: { tabId: tab.id, allFrames: true }, func: selectLinksInPage },
      (results) => {
        if (chrome.runtime.lastError) {
          callback({ error: "Cannot read this page" });
          return;
        }
        const found = [];
        for (const frame of results || []) {
          if (Array.isArray(frame.result)) found.push(...frame.result);
        }
        callback({ urls: [...new Set(found)] });
      }
    );
  });
}

document.addEventListener("DOMContentLoaded", async () => {
  setStatus(await ping());
});

sendButton.addEventListener("click", () => {
  sendButton.disabled = true;
  showLabel(sendButton, "Collecting…", 0, SEND_LABEL);
  collectSelectedLinks(async ({ urls = [], error }) => {
    if (error) {
      finish(sendButton, error, 3000, SEND_LABEL);
      return;
    }
    if (!urls.length) {
      finish(sendButton, "No links selected", 3000, SEND_LABEL);
      return;
    }
    try {
      await postUrls(urls);
      setStatus(true);
      finish(sendButton, `Sent ${urls.length} link(s)`, 2500, SEND_LABEL);
    } catch {
      setStatus(false);
      finish(sendButton, "DVIDM is offline", 3000, SEND_LABEL);
    }
  });
});

copyButton.addEventListener("click", () => {
  copyButton.disabled = true;
  showLabel(copyButton, "Collecting…", 0, COPY_LABEL);
  collectSelectedLinks(async ({ urls = [], error }) => {
    if (error) {
      finish(copyButton, error, 3000, COPY_LABEL);
      return;
    }
    if (!urls.length) {
      finish(copyButton, "No links selected", 3000, COPY_LABEL);
      return;
    }
    const ok = await copyText(urls.join("\n"));
    finish(
      copyButton,
      ok ? `Copied ${urls.length} link(s)` : "Copy failed",
      ok ? 2500 : 3000,
      COPY_LABEL
    );
  });
});

// Runs inside the page via chrome.scripting.executeScript.
function selectLinksInPage() {
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
