/*
 * The language the interface is read in: the browser's to begin with, or one
 * picked by hand. The same choice on the door and in one's own settings.
 */

import { Picker } from "./panel";
import { languageChoiceOf } from "../i18n";
import { useSettings } from "../settings";

export function LanguagePicker() {
  const { t, languageChoice, setLanguage } = useSettings();
  return (
    <Picker
      label={t("nav.language")}
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
