import ru from "./ru.json";
import en from "./en.json";
import zhCN from "./zh-CN.json";

export type Locale = "ru" | "en" | "zh-CN";
export type MessageKey = keyof typeof ru;

const catalogs: Record<Locale, Record<string, string>> = {
  ru,
  en,
  "zh-CN": zhCN,
};

export function t(
  locale: Locale,
  key: MessageKey,
  args?: Record<string, string | number>,
): string {
  const template = catalogs[locale][key] ?? catalogs.ru[key] ?? key;
  if (!args) return template;
  return template.replace(/\{(\w+)\}/g, (_, name: string) =>
    String(args[name] ?? `{${name}}`),
  );
}

export const locales: { id: Locale; label: string }[] = [
  { id: "ru", label: "RU" },
  { id: "en", label: "EN" },
  { id: "zh-CN", label: "中文" },
];
