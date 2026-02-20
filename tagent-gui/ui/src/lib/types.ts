export interface TranslationResult {
  original: string;
  translated: string;
  from_lang: string;
  to_lang: string;
  is_dictionary: boolean;
  dictionary?: DictionaryData;
}

export interface DictionaryData {
  word: string;
  entries: PosEntry[];
}

export interface PosEntry {
  part_of_speech: string;
  definitions: DefData[];
}

export interface DefData {
  text: string;
  synonyms: string[];
}

export interface ConfigData {
  source_language: string;
  target_language: string;
  show_dictionary: boolean;
  copy_to_clipboard: boolean;
  save_translation_history: boolean;
  translate_hotkey: string;
  speech_hotkey: string;
  enable_text_to_speech: boolean;
  enable_speech_hotkey: boolean;
  translate_provider: string;
  history_file: string;
}

export interface LanguageEntry {
  name: string;
  code: string;
}
