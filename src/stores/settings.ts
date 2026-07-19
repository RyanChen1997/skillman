import { defineStore } from "pinia";
import { ref, type Ref } from "vue";
import { getSetting, setSetting } from "../lib/tauri";

export type ThemeMode = "light" | "dark" | "system";

const THEME_KEY = "theme";

let systemListenerRegistered = false;

function isThemeMode(v: string | null): v is ThemeMode {
  return v === "light" || v === "dark" || v === "system";
}

function systemPrefersDark(): boolean {
  return window.matchMedia("(prefers-color-scheme: dark)").matches;
}

function effectiveTheme(mode: ThemeMode): "light" | "dark" {
  if (mode === "system") return systemPrefersDark() ? "dark" : "light";
  return mode;
}

export const useSettingsStore = defineStore("settings", () => {
  const showUninstalled = ref(false);
  const theme: Ref<ThemeMode> = ref("system");
  const loaded = ref(false);

  function applyTheme() {
    document.documentElement.dataset.theme = effectiveTheme(theme.value);
  }

  function listenToSystemChanges() {
    if (systemListenerRegistered) return;
    systemListenerRegistered = true;
    const mq = window.matchMedia("(prefers-color-scheme: dark)");
    const handler = () => { if (theme.value === "system") applyTheme(); };
    if (mq.addEventListener) mq.addEventListener("change", handler);
    else mq.addListener(handler);
  }

  async function load() {
    const show = await getSetting("show_uninstalled_agents");
    showUninstalled.value = show === "true";

    const t = await getSetting(THEME_KEY);
    theme.value = isThemeMode(t) ? t : "system";
    applyTheme();
    listenToSystemChanges();

    loaded.value = true;
  }

  async function setShowUninstalled(v: boolean) {
    showUninstalled.value = v;
    await setSetting("show_uninstalled_agents", v ? "true" : "false");
  }

  async function setTheme(v: ThemeMode) {
    theme.value = v;
    applyTheme();
    await setSetting(THEME_KEY, v);
  }

  return {
    showUninstalled,
    theme,
    loaded,
    load,
    setShowUninstalled,
    setTheme,
    applyTheme,
  };
});
