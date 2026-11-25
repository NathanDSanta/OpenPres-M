/** A language there can be a translation track in (the server's list). */
export interface TranslationLanguage {
  /** ISO 639-1: "es". */
  code: string;
  /** In the language itself, as phones show it: "Español". */
  name: string;
  /** In English: "Spanish". */
  english: string;
}

/**
 * Manual: an interpreter speaks into a microphone that goes to `bus`. Automatic: the
 * computer translates `source_bus` by itself (speech recognition, translation, a voice; not
 * available yet).
 */
export type TranslationMode = "manual" | "automatic";

/** One language's track: what phones hear in that language. */
export interface TranslationTrack {
  language: string;
  /** Phones can listen while it's on. */
  on: boolean;
  mode: TranslationMode;
  /** Manual: the interpreter's bus (1-16). */
  bus: number | null;
  /** Automatic: the bus to translate. */
  source_bus: number | null;
}

/** A track as the server reports it, with what it does now. */
export interface TranslationTrackStatus extends TranslationTrack {
  /** Phones hear it now. */
  live: boolean;
  /** Why phones hear nothing although it's on. */
  problem: string | null;
  /** Phones (and browsers) listening now. */
  listeners: number;
  /** Phones get captions too (the automatic mode, later). */
  captions: boolean;
}

export interface TranslationView {
  languages: TranslationLanguage[];
  tracks: TranslationTrackStatus[];
  /** Phones can reach the server (it accepts other devices). */
  reachable: boolean;
}

/** What phones (and the viewer page) see: `/translation/tracks`. */
export interface PublicTranslationTrack {
  language: string;
  name: string;
  live: boolean;
  captions: boolean;
}
