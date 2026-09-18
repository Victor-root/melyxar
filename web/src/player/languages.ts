/*
 * Turning a language code into a word a viewer reads.
 *
 * Files carry three letter codes, and two competing sets of them at that: the
 * server settles on one so a picker never shows the same language twice, and
 * this turns that one back into a name, in whichever language the interface is
 * speaking. A code nobody recognises is shown as it is rather than dropped,
 * since a picker with a blank entry is worse than one with an odd word in it.
 */

/** The three letter codes the server settles on, and their two letter twins. */
const PAIRS: Record<string, string> = {
  eng: "en",
  fre: "fr",
  ger: "de",
  spa: "es",
  ita: "it",
  jpn: "ja",
  dut: "nl",
  por: "pt",
  rus: "ru",
  chi: "zh",
  ara: "ar",
  kor: "ko",
  pol: "pl",
  swe: "sv",
  dan: "da",
  fin: "fi",
  nor: "no",
  tur: "tr",
  ces: "cs",
  cze: "cs",
  hun: "hu",
  ell: "el",
  gre: "el",
  heb: "he",
  hin: "hi",
  tha: "th",
  ukr: "uk",
  vie: "vi",
};

/**
 * The languages a library can ask a provider to describe its films in.
 *
 * The two letter codes above, which are the ones a provider takes, without the
 * duplicates the three letter side carries. A list rather than a text field so
 * that nobody can leave a library asking for a language nothing answers in,
 * and the names come out in whatever the interface is speaking.
 */
export const METADATA_LANGUAGES: string[] = [...new Set(Object.values(PAIRS))].sort();

export function languageName(code: string, speaking: string): string {
  const short = PAIRS[code.toLowerCase()] ?? code.toLowerCase();
  try {
    const names = new Intl.DisplayNames([speaking], { type: "language" });
    const name = names.of(short);
    // A browser hands back what it was given when it knows nothing better.
    if (name && name.toLowerCase() !== short) {
      return name.charAt(0).toUpperCase() + name.slice(1);
    }
  } catch {
    // An interface language this browser has no names for is not a reason to
    // show nothing.
  }
  return code;
}
