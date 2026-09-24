import { en } from "./en";
import { ru } from "./ru";
export type Language = "en" | "ru";
export type MessageKey = keyof typeof en;
let currentLanguage: Language = "en";
export function setInterfaceLanguage(language: Language) { currentLanguage = language; }
export function currentLocale() { return localeName(currentLanguage); }
type Tokens<S extends string> = S extends `${string}{${infer P}}${infer Rest}` ? P | Tokens<Rest> : never;
type Arguments<K extends MessageKey> = [Tokens<(typeof en)[K]>] extends [never] ? [] : [Record<Tokens<(typeof en)[K]>, string | number>];
export function translate<K extends MessageKey>(language: Language, key: K, ...args: Arguments<K>): string {
  const pattern = (language === "ru" ? ru[key] : en[key]) ?? en[key];
  const params = args[0] as Record<string, string | number> | undefined;
  return pattern.replace(/\{([^{}]+)\}/g, (_, name: string) => String(params?.[name] ?? `{${name}}`));
}
export const localeName = (language: Language) => language === "ru" ? "ru-RU" : "en-US";
export function t<K extends MessageKey>(key: K, ...args: Arguments<K>) { return translate(currentLanguage, key, ...args); }
export function translator(language: Language) {
  return <K extends MessageKey>(key: K, ...args: Arguments<K>) => translate(language, key, ...args);
}
