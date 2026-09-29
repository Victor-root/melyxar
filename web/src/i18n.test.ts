import { describe, expect, it } from "vitest";
import { languageChoiceOf, languageOf } from "./i18n";

describe("the language the interface is read in", () => {
  it("follows the browser when automatic", () => {
    expect(languageOf("auto", "fr-FR")).toBe("fr");
    expect(languageOf("auto", "de-DE")).toBe("en");
  });

  it("keeps a language picked by hand whatever the browser says", () => {
    expect(languageOf("en", "fr-FR")).toBe("en");
    expect(languageOf("fr", "en-US")).toBe("fr");
  });

  it("follows the browser for anything it does not know", () => {
    expect(languageChoiceOf(null)).toBe("auto");
    expect(languageChoiceOf("de")).toBe("auto");
    expect(languageChoiceOf("fr")).toBe("fr");
  });
});
