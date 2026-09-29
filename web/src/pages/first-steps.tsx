/*
 * The first steps of a brand new server, taken once its first account exists.
 *
 * Laid out like the administration, a list of the steps down the side and the
 * step itself across the rest of the screen, because that is where somebody
 * finds these same settings again later. The installer only installs:
 * everything a server is set up with is said here, with the forms the
 * administration uses.
 */

import { useState } from "react";
import type { ComponentType } from "react";
import type { Library } from "../api";
import { useTold } from "../asking";
import { PageBackdrop } from "../components/backdrop";
import { LanguagePicker } from "../components/language-picker";
import { ThemeToggle } from "../components/theme-toggle";
import { Panel } from "../components/panel";
import { refusalKey } from "../i18n";
import { AccountIcon, FolderIcon, KindIcon, PlayIcon, TickIcon } from "../icons";
import type { IconProps } from "../icons";
import { NewLibrary } from "./admin/libraries";
import { useSettings } from "../settings";

type Step = "account" | "libraries" | "ready";

const STEPS: { step: Step; icon: ComponentType<IconProps>; label: string }[] = [
  { step: "account", icon: AccountIcon, label: "first_steps.step.account" },
  { step: "libraries", icon: FolderIcon, label: "first_steps.step.libraries" },
  { step: "ready", icon: PlayIcon, label: "first_steps.step.ready" },
];

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
  /* The account is made on the door, so the steps open on the one after. */
  const [step, setStep] = useState<Step>("libraries");
  const at = STEPS.findIndex((one) => one.step === step);
  const finishing = useTold(onFinish);

  return (
    <div className="sectioned first-steps">
      <PageBackdrop />

      <aside className="side">
        <nav className="side-list" aria-label={t("first_steps.title")}>
          <div className="side-group">
            <span className="side-group-name">{t("first_steps.steps")}</span>
            {STEPS.map((one, index) => {
              const StepIcon = index < at ? TickIcon : one.icon;
              return (
                <button
                  key={one.step}
                  type="button"
                  className={`side-line${index === at ? " active" : ""}${index < at ? " side-line-done" : ""}`}
                  // The account is behind, and what comes after this step is
                  // reached by finishing it: only the steps already passed
                  // that can be taken again are offered.
                  disabled={one.step === "account" || index > at}
                  aria-current={index === at ? "step" : undefined}
                  onClick={() => setStep(one.step)}
                >
                  <StepIcon size={20} />
                  <span className="side-line-name">{t(one.label)}</span>
                </button>
              );
            })}
          </div>
          <div className="side-group">
            <span className="side-group-name">{t("first_steps.declared")}</span>
            {libraries.length === 0 ? (
              <span className="first-steps-none">{t("first_steps.none_yet")}</span>
            ) : (
              libraries.map((library) => (
                <span className="first-steps-declared-line" key={library.id}>
                  <KindIcon kind={library.kind} size={18} />
                  <span className="side-line-name">{library.name}</span>
                </span>
              ))
            )}
          </div>
        </nav>
        {/* The two settings of the door stay in reach all the way through:
            the settings pages are behind these steps. */}
        <div className="side-foot first-steps-foot-side">
          <span>{t("first_steps.foot", { at: at + 1, of: STEPS.length })}</span>
          <LanguagePicker />
          <ThemeToggle />
        </div>
      </aside>

      <main className="first-steps-page">
        <header className="page-head">
          <div className="page-head-line">
            <span className="page-head-mark" aria-hidden="true">
              {step === "libraries" ? <FolderIcon size={24} /> : <PlayIcon size={24} />}
            </span>
            <div className="page-head-words">
              <h1>{t(step === "libraries" ? "first_steps.title" : "first_steps.ready_title")}</h1>
              <p>{t(step === "libraries" ? "first_steps.lead" : "first_steps.ready_lead")}</p>
            </div>
          </div>
        </header>

        {step === "libraries" ? (
          <Libraries libraries={libraries} onDeclared={onDeclared} onNext={() => setStep("ready")} />
        ) : (
          <>
            {finishing.failure && (
              <p className="panel-notice panel-notice-trouble">
                {t(refusalKey(finishing.failure.code))}
              </p>
            )}
            <div className="first-steps-foot">
              <button className="button" onClick={() => setStep("libraries")}>
                {t("first_steps.back")}
              </button>
              <button
                className="button button-accent"
                disabled={finishing.busy}
                onClick={() => void finishing.tell()}
              >
                {t("first_steps.finish")}
              </button>
            </div>
          </>
        )}
      </main>
    </div>
  );
}

/** Declaring the libraries: the form of the administration on one side, what
 *  is already declared on the other. */
function Libraries({
  libraries,
  onDeclared,
  onNext,
}: {
  libraries: Library[];
  onDeclared: () => void;
  onNext: () => void;
}) {
  const { t } = useSettings();
  /* Open straight away on a server with nothing in it: declaring a library
     is the whole of what this step is for. */
  const [adding, setAdding] = useState(libraries.length === 0);
  const [refused, setRefused] = useState<string | null>(null);

  return (
    <>
      {refused && <p className="panel-notice panel-notice-trouble">{t(refused)}</p>}

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
        <Panel
          icon={FolderIcon}
          title={t("settings.add_library")}
          lead={t(libraries.length === 0 ? "first_steps.another_why" : "first_steps.scanning")}
        >
          <button className="button button-accent first-steps-add" onClick={() => setAdding(true)}>
            <FolderIcon size={18} />
            {t("first_steps.add_another")}
          </button>
        </Panel>
      )}

      <div className="first-steps-foot">
        <span>{t("first_steps.later")}</span>
        <button className="button button-accent" onClick={onNext}>
          {t(libraries.length === 0 ? "first_steps.skip" : "first_steps.next")}
        </button>
      </div>
    </>
  );
}
