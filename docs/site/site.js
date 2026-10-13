"use strict";

const root = document.documentElement;
const themeButton = document.querySelector(".theme-toggle");

function setTheme(theme) {
  root.dataset.theme = theme;
  const next = theme === "dark" ? "light" : "dark";
  themeButton.textContent = `[${next}]`;
  themeButton.setAttribute("aria-label", `Switch to ${next} theme`);
  document.querySelector('meta[name="theme-color"]').content = theme === "dark" ? "#111415" : "#f6f7f2";
}

try {
  const savedTheme = localStorage.getItem("repostatus-site-theme");
  if (savedTheme === "light" || savedTheme === "dark") setTheme(savedTheme);
} catch { /* The site also works when browser storage is unavailable. */ }

themeButton.addEventListener("click", () => {
  const theme = root.dataset.theme === "dark" ? "light" : "dark";
  setTheme(theme);
  try { localStorage.setItem("repostatus-site-theme", theme); } catch { /* Optional preference. */ }
});

const refreshButton = document.querySelector("#refresh-demo");
const demoStatus = document.querySelector("#demo-status");
refreshButton.addEventListener("click", () => {
  refreshButton.disabled = true;
  refreshButton.textContent = "[checking…]";
  demoStatus.textContent = "checking sample repositories…";
  window.setTimeout(() => {
    refreshButton.disabled = false;
    refreshButton.textContent = "[r] refresh preview";
    demoStatus.textContent = "preview checked · just now";
  }, 650);
});

document.querySelector("#copy-commands").addEventListener("click", async (event) => {
  const button = event.currentTarget;
  const commands = document.querySelector("#install-commands");
  const status = document.querySelector("#copy-status");
  try {
    await navigator.clipboard.writeText(commands.textContent.trim());
    button.textContent = "[copied]";
    status.textContent = "Commands copied. Paste them into your terminal to get started.";
    window.setTimeout(() => { button.textContent = "[copy]"; }, 2000);
  } catch {
    const range = document.createRange();
    range.selectNodeContents(commands);
    const selection = window.getSelection();
    selection.removeAllRanges();
    selection.addRange(range);
    status.textContent = "Commands selected. Press Ctrl+C or ⌘C to copy.";
  }
});

document.addEventListener("keydown", (event) => {
  if (event.ctrlKey || event.metaKey || event.altKey || event.repeat || event.target.closest("input, textarea, select, [contenteditable]")) return;
  const key = event.key.toLowerCase();
  const target = { "1": "features", "2": "install" }[key];
  if (target) { event.preventDefault(); window.location.hash = target; }
  if (key === "t") themeButton.click();
  if (key === "r" && !refreshButton.disabled) refreshButton.click();
});
