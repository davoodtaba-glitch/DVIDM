// Offscreen documents are not focused, so navigator.clipboard.writeText can be
// rejected. A textarea plus execCommand works with the clipboardWrite permission.
function copyWithSelection(text) {
  const area = document.createElement("textarea");
  area.value = text;
  area.style.position = "fixed";
  area.style.top = "-1000px";
  area.style.opacity = "0";
  document.body.appendChild(area);
  area.focus();
  area.select();
  let ok = false;
  try {
    ok = document.execCommand("copy");
  } catch (err) {
    // fall through to the async API
  }
  area.remove();
  return ok;
}

async function copyText(text) {
  if (copyWithSelection(text)) return true;
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch (err) {
    return false;
  }
}

chrome.runtime.onMessage.addListener((message, sender, sendResponse) => {
  if (!message || message.type !== "dvidm-copy") return;
  copyText(String(message.text || "")).then((ok) => sendResponse({ ok }));
  return true;
});
