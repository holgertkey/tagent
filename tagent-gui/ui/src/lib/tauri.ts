import { invoke } from "@tauri-apps/api/core";
import type { TranslationResult, ConfigData, LanguageEntry } from "./types";

export const translate = (
  text: string,
  from_lang: string,
  to_lang: string
) =>
  invoke<TranslationResult>("translate", { text, fromLang: from_lang, toLang: to_lang });

export const getConfig = () => invoke<ConfigData>("get_config");

export const saveConfig = (config: ConfigData) =>
  invoke<void>("save_config", { config });

export const getLanguages = () => invoke<LanguageEntry[]>("get_languages");

export const speak = (text: string) => invoke<void>("speak", { text });

export const swapLanguages = () =>
  invoke<[string, string]>("swap_languages");

export const copyToClipboard = (text: string) =>
  invoke<void>("copy_to_clipboard", { text });
