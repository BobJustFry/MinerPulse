(function () {
  const LOCALES = [
    { id: "ru", label: "Русский" },
    { id: "en", label: "English" },
    { id: "zh-CN", label: "中文" },
  ];

  const STORAGE_LOCALE = "mpulse_locale";
  const STORAGE_THEME = "mpulse_theme";

  let locale = "ru";
  let messages = {};
  let theme = "light";

  function htmlLang(id) {
    return id === "zh-CN" ? "zh-Hans" : id;
  }

  function interpolate(text, vars) {
    if (!vars) return text;
    return Object.entries(vars).reduce(
      (acc, [key, value]) => acc.replaceAll(`{${key}}`, String(value)),
      text,
    );
  }

  function t(key, vars) {
    const raw = messages[key] ?? key;
    return interpolate(raw, vars);
  }

  function applyStaticI18n() {
    document.title = t("meta.title");

    document.querySelectorAll("[data-i18n]").forEach((el) => {
      el.textContent = t(el.dataset.i18n);
    });

    document.querySelectorAll("[data-i18n-placeholder]").forEach((el) => {
      el.placeholder = t(el.dataset.i18nPlaceholder);
    });

    document.querySelectorAll("[data-i18n-aria]").forEach((el) => {
      el.setAttribute("aria-label", t(el.dataset.i18nAria));
    });

    const select = document.getElementById("locale-select");
    if (select) select.value = locale;

    updateThemeToggleLabel();
  }

  function updateThemeToggleLabel() {
    const btn = document.getElementById("theme-toggle");
    if (!btn) return;
    btn.textContent = theme === "dark" ? t("theme.light") : t("theme.dark");
    btn.setAttribute(
      "aria-label",
      theme === "dark" ? t("theme.light") : t("theme.dark"),
    );
  }

  function applyTheme(nextTheme) {
    theme = nextTheme === "dark" ? "dark" : "light";
    document.documentElement.dataset.theme = theme;
    localStorage.setItem(STORAGE_THEME, theme);
    updateThemeToggleLabel();
  }

  function initTheme() {
    const stored = localStorage.getItem(STORAGE_THEME);
    if (stored === "light" || stored === "dark") {
      applyTheme(stored);
      return;
    }
    const prefersDark = window.matchMedia("(prefers-color-scheme: dark)").matches;
    applyTheme(prefersDark ? "dark" : "light");
  }

  function toggleTheme() {
    applyTheme(theme === "dark" ? "light" : "dark");
  }

  function detectSystemLocale() {
    const langs = navigator.languages?.length ? navigator.languages : [navigator.language];
    for (const raw of langs) {
      const tag = String(raw || "").toLowerCase();
      if (tag.startsWith("zh")) return "zh-CN";
      if (tag.startsWith("ru")) return "ru";
      if (tag.startsWith("en")) return "en";
    }
    return "en";
  }

  async function loadLocale(nextLocale, persist = true) {
    if (!LOCALES.some((item) => item.id === nextLocale)) return;
    const res = await fetch(`/i18n/${nextLocale}.json`);
    if (!res.ok) throw new Error(`locale_load_failed:${nextLocale}`);
    messages = await res.json();
    locale = nextLocale;
    if (persist) {
      localStorage.setItem(STORAGE_LOCALE, locale);
    }
    document.documentElement.lang = htmlLang(locale);
    applyStaticI18n();
    document.dispatchEvent(new CustomEvent("mpulse:locale", { detail: { locale } }));
  }

  function humanError(code) {
    const key = `error.${code}`;
    return messages[key] ? t(key) : code || t("error.generic");
  }

  function populateLocaleSelect() {
    const select = document.getElementById("locale-select");
    if (!select || select.options.length > 0) return;
    LOCALES.forEach((item) => {
      const option = document.createElement("option");
      option.value = item.id;
      option.textContent = item.label;
      select.appendChild(option);
    });
  }

  async function init() {
    populateLocaleSelect();
    initTheme();

    const storedLocale = localStorage.getItem(STORAGE_LOCALE);
    const hasStoredLocale = LOCALES.some((item) => item.id === storedLocale);
    const initialLocale = hasStoredLocale ? storedLocale : detectSystemLocale();

    await loadLocale(initialLocale, hasStoredLocale);

    document.getElementById("locale-select")?.addEventListener("change", (event) => {
      loadLocale(event.target.value, true).catch(console.error);
    });

    document.getElementById("theme-toggle")?.addEventListener("click", toggleTheme);
  }

  window.MPulseI18n = {
    init,
    t,
    humanError,
    getLocale: () => locale,
    getTheme: () => theme,
    LOCALES,
  };

  init()
    .then(() => document.dispatchEvent(new Event("mpulse:ready")))
    .catch((err) => {
      console.error(err);
      document.dispatchEvent(new Event("mpulse:ready"));
    });
})();
