/*
 * What the server knows about the works, and where it learns it from: the
 * language each library is described in, the files sitting beside the films,
 * and the provider it asks.
 */

import { PageHead, Panel, Picker, Setting, Toggle } from "../../components/panel";
import { FolderIcon, IdentifyIcon, KindIcon, TagIcon } from "../../icons";
import { languageName, METADATA_LANGUAGES } from "../../languages";
import { useLibraries } from "../../libraries";
import { useLibraryEditing } from "../../screens/declaring";
import { useLibraryWork } from "../../screens/settings";
import { useSettings } from "../../settings";

export function AdminMetadata() {
  const { t, language } = useSettings();
  const { all: libraries, refresh } = useLibraries();
  const { refused, outcome, settle } = useLibraryEditing(refresh);
  const work = useLibraryWork();
  const languages = METADATA_LANGUAGES.map((code) => [code, languageName(code, language)] as const);

  return (
    <>
      <PageHead lead={t("admin.metadata_lead")} />

      {refused && <p className="panel-notice panel-notice-trouble">{t(refused)}</p>}
      {/* Somebody who just changed the language of a library is owed how
          many works that sends back to the provider. */}
      {outcome?.kind === "asked_about_again" && (
        <p className="panel-notice">{t("settings.asked_about_again", { count: outcome.count })}</p>
      )}

      <Panel icon={TagIcon} title={t("settings.metadata_language")} lead={t("settings.metadata_language_why")}>
        {libraries.map((library) => (
          <Setting
            key={library.id}
            label={library.name}
            why={t(`kind.${library.kind}`)}
          >
            <KindIcon kind={library.kind} size={18} />
            <Picker
              label={t("settings.metadata_language")}
              value={library.metadata_language}
              options={languages}
              onPick={(metadata_language) => settle(library, { metadata_language })}
            />
          </Setting>
        ))}
      </Panel>

      <div className="panels">
        {work.kept && (
          <Panel icon={FolderIcon} title={t("settings.companion_files")} lead={t("settings.companion_files_why")}>
            <Setting label={t("settings.read_companion_files")}>
              <Toggle
                label={t("settings.read_companion_files")}
                checked={work.kept.read_companion_files}
                onChange={(read_companion_files) => work.setTo({ read_companion_files })}
              />
            </Setting>
            <Setting label={t("admin.write_companion_files")} why={t("admin.write_companion_files_why")} soon>
              <Toggle label={t("admin.write_companion_files")} checked={false} onChange={() => {}} disabled />
            </Setting>
          </Panel>
        )}

        <Panel icon={IdentifyIcon} title={t("admin.provider")} lead={t("admin.provider_lead")} soon>
          <Setting label={t("admin.provider_state")} soon>
            <span className="state-pill state-attention">
              <span className="state-dot" aria-hidden="true" />–
            </span>
          </Setting>
          <Setting label={t("admin.unnamed")} why={t("admin.unnamed_why")} soon>
            <button className="button button-small" disabled>
              {t("admin.see")}
            </button>
          </Setting>
        </Panel>
      </div>

      {work.failed && (
        <p className="panel-notice panel-notice-trouble">
          {t(work.failed === "not_kept" ? "settings.not_kept" : "error.unreachable")}
        </p>
      )}
    </>
  );
}
