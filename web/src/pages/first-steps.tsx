/*
 * The first steps of a brand new server, taken once its first account exists:
 * the libraries it is to hold.
 *
 * The installer only installs. Everything a server is set up with is said
 * here, in the browser, with the same form the administration uses, so there
 * is one way of declaring a library and it is the one somebody finds again
 * later.
 */

import { useState } from "react";
import type { Library } from "../api";
import { useTold } from "../asking";
import { refusalKey } from "../i18n";
import { FolderIcon, KindIcon } from "../icons";
import { NewLibrary } from "./admin/libraries";
import { useSettings } from "../settings";

export function FirstSteps({
  libraries,
  onDeclared,
  onFinish,
}: {
  libraries: Library[];
  onDeclared: () => void;
  onFinish: () => Promise<void>;
}) {
  const { t } = useSettings();
  /* Open straight away on a server with nothing in it: declaring a library
     is the whole of what this screen is for. */
  const [adding, setAdding] = useState(libraries.length === 0);
  const [refused, setRefused] = useState<string | null>(null);
  const finishing = useTold(onFinish);

  return (
    <main className="page first-steps">
      <header className="first-steps-head">
        <span className="page-head-mark" aria-hidden="true">
          <FolderIcon size={24} />
        </span>
        <div>
          <h1>{t("first_steps.title")}</h1>
          <p>{t("first_steps.lead")}</p>
        </div>
      </header>

      {(refused || finishing.failure) && (
        <p className="panel-notice panel-notice-trouble">
          {t(refused ?? refusalKey(finishing.failure!.code))}
        </p>
      )}

      {libraries.length > 0 && (
        <div className="lines">
          {libraries.map((library) => (
            <div className="line" key={library.id}>
              <span className="line-mark" aria-hidden="true">
                <KindIcon kind={library.kind} size={18} />
              </span>
              <span className="line-words">
                <span className="line-name">{library.name}</span>
                <span className="line-note">{t(`library.kind.${library.kind}`)}</span>
              </span>
            </div>
          ))}
        </div>
      )}

      {adding ? (
        <NewLibrary
          onDone={() => {
            setAdding(false);
            setRefused(null);
            onDeclared();
          }}
          onCancel={() => setAdding(false)}
          onRefused={setRefused}
        />
      ) : (
        <button className="button first-steps-add" onClick={() => setAdding(true)}>
          <FolderIcon size={18} />
          {t(libraries.length === 0 ? "settings.add_library" : "first_steps.add_another")}
        </button>
      )}

      <div className="first-steps-foot">
        <p>{t("first_steps.later")}</p>
        <button
          className="button button-accent"
          disabled={finishing.busy}
          onClick={() => void finishing.tell()}
        >
          {t(libraries.length === 0 ? "first_steps.skip" : "first_steps.finish")}
        </button>
      </div>
    </main>
  );
}
