import { apiGet, apiPut } from "./client";
import type { PublicTranslationTrack, TranslationTrack, TranslationView } from "@/types";

/** Translation tracks (the server's translation.rs). */
export const translationApi = {
  get: () => apiGet<TranslationView>("/translation"),
  replace: (tracks: TranslationTrack[]) => apiPut<TranslationView>("/translation", { tracks }),
  /** Public (phones, the viewer page): every track and whether it can be heard now. */
  tracks: () => apiGet<{ tracks: PublicTranslationTrack[] }>("/translation/tracks"),
  /** A track's sound (see BusListener). Speech: 24 kHz mono by default. */
  listenPath: (language: string) => `/translation/tracks/${encodeURIComponent(language)}/listen`,
};
