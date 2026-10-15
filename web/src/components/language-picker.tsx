/*
 * The language the interface is read in: the browser's to begin with, or one
 * picked by hand. The same choice on the door and in one's own settings.
 */

import { Picker } from "./panel";
import { languageChoiceOf } from "../i18n";
import { LanguagesIcon } from "../icons";
import { useSettings } from "../settings";

/** Quiet where it stands alone, with nothing saying what it is for: its icon
 *  says it, and the browser's choice is said in one word. */
export function LanguagePicker({ quiet = false }: { quiet?: boolean }) {
  const { t, languageChoice, setLanguage } = useSettings();
  return (
    <Picker
      label={t("nav.language")}
      icon={quiet ? <LanguagesIcon size={16} /> : undefined}
      fieldWording={quiet && languageChoice === "auto" ? t("language.auto_short") : undefined}
      value={languageChoice}
      onPick={(picked) => setLanguage(languageChoiceOf(picked))}
      options={[
        ["auto", t("language.auto")],
        ["en", "English"],
        ["fr", "Français"],
      ]}
    />
  );
}
