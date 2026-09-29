/*
 * The first steps of a brand new server, taken once its first account exists.
 *
 * Laid out like the administration, a list of the steps down the side and the
 * step itself across the rest of the screen, because that is where somebody
 * finds these same settings again later: every step is the administration's
 * own panel, and every one of them can be passed over. The installer only
 * installs; everything a server is set up with is said here.
 */

import { useState } from "react";
import type { ComponentType, ReactNode } from "react";
import type { Library } from "../api";
import { useTold } from "../asking";
import { PageBackdrop } from "../components/backdrop";
import { LanguagePicker } from "../components/language-picker";
import { Panel, Setting } from "../components/panel";
import { ThemeToggle } from "../components/theme-toggle";
import { refusalKey } from "../i18n";
import {
  AccountIcon,
  ClockIcon,
  FolderIcon,
  GraphicsCardIcon,
  KindIcon,
  PlayIcon,
  ServerIcon,
  ShieldIcon,
  StarIcon,
  TickIcon,
} from "../icons";
import type { IconProps } from "../icons";
import { useActivityScreen } from "../screens/activity";
import { useSettings } from "../settings";
import { NewLibrary } from "./admin/libraries";
import { OnlineSubtitlesPanel } from "./admin/online-subtitles";
import { RatingsPanel } from "./admin/ratings";
import { ServerPanel } from "./admin/settings";
import { ScheduledTasksPanel } from "./admin/tasks";
import { CardPanel } from "./admin/transcoding";

type Step = "account" | "server" | "libraries" | "services" | "card" | "tasks" | "access" | "ready";

const STEPS: { step: Step; icon: ComponentType<IconProps> }[] = [
  { step: "account", icon: AccountIcon },
  { step: "server", icon: ServerIcon },
  { step: "libraries", icon: FolderIcon },
  { step: "services", icon: StarIcon },
  { step: "card", icon: GraphicsCardIcon },
  { step: "tasks", icon: ClockIcon },
  { step: "access", icon: ShieldIcon },
  { step: "ready", icon: PlayIcon },
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
  const [at, setAt] = useState(1);
  /* The steps left behind, whether they were gone through or passed over. */
  const [behind, setBehind] = useState(1);
  const finishing = useTold(onFinish);
  const step = STEPS[at];
  const StepIcon = step.icon;

  const goTo = (index: number) => {
    setAt(index);
    setBehind((was) => Math.max(was, index));
  };

  return (
    <div className="sectioned first-steps">
      <PageBackdrop />

      <aside className="side">
        <nav className="side-list" aria-label={t("first_steps.steps")}>
          <div className="side-group">
            <span className="side-group-name">{t("first_steps.steps")}</span>
            {STEPS.map((one, index) => {
              const LineIcon = index < behind && index !== at ? TickIcon : one.icon;
              return (
                <button
                  key={one.step}
                  type="button"
                  className={`side-line${index === at ? " active" : ""}${
                    index < behind && index !== at ? " side-line-done" : ""
                  }`}
                  // Every step can be opened in any order; the account alone
                  // is behind for good, made on the door.
                  disabled={one.step === "account"}
                  aria-current={index === at ? "step" : undefined}
                  onClick={() => goTo(index)}
                >
                  <LineIcon size={20} />
                  <span className="side-line-name">{t(`first_steps.step.${one.step}`)}</span>
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
              <StepIcon size={24} />
            </span>
            <div className="page-head-words">
              <h1>{t(`first_steps.${step.step}.title`)}</h1>
              <p>{t(`first_steps.${step.step}.lead`)}</p>
            </div>
          </div>
        </header>

        {finishing.failure && (
          <p className="panel-notice panel-notice-trouble">{t(refusalKey(finishing.failure.code))}</p>
        )}

        <StepBody step={step.step} libraries={libraries} onDeclared={onDeclared} />

        <div className="first-steps-foot">
          <span>{t("first_steps.later")}</span>
          <span className="first-steps-buttons">
            {at > 1 && (
              <button className="button" onClick={() => goTo(at - 1)}>
                {t("first_steps.back")}
              </button>
            )}
            {step.step === "ready" ? (
              <button
                className="button button-accent"
                disabled={finishing.busy}
                onClick={() => void finishing.tell()}
              >
                {t("first_steps.finish")}
              </button>
            ) : (
              <>
                <button className="button button-quiet" onClick={() => goTo(at + 1)}>
                  {t("first_steps.skip")}
                </button>
                <button className="button button-accent" onClick={() => goTo(at + 1)}>
                  {t("first_steps.next")}
                </button>
              </>
            )}
          </span>
        </div>
      </main>
    </div>
  );
}

/** What each step holds: the administration's own panels. */
function StepBody({
  step,
  libraries,
  onDeclared,
}: {
  step: Step;
  libraries: Library[];
  onDeclared: () => void;
}): ReactNode {
  switch (step) {
    case "server":
      return <ServerPanel />;
    case "libraries":
      return <Libraries libraries={libraries} onDeclared={onDeclared} />;
    case "services":
      return (
        <div className="first-steps-pair">
          <RatingsPanel />
          <OnlineSubtitlesPanel />
        </div>
      );
    case "card":
      return <CardPanel />;
    case "tasks":
      return <Tasks />;
    case "access":
      return <Access />;
    default:
      return null;
  }
}

/** Declaring the libraries, with the form of the administration. */
function Libraries({ libraries, onDeclared }: { libraries: Library[]; onDeclared: () => void }) {
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
    </>
  );
}

/** The hours the heavy work runs at, as the administration sets them. */
function Tasks() {
  const { tasks, startEveryTask, startTask, schedule } = useActivityScreen();
  return (
    <ScheduledTasksPanel
      tasks={tasks}
      onStartEvery={startEveryTask}
      onStart={startTask}
      onSchedule={schedule}
    />
  );
}

/**
 * How this server is reached. Said rather than set: the server answers on the
 * address it was installed with, and the ways of reaching it from outside are
 * a proxy put in front of it, until the server offers its own.
 */
function Access() {
  const { t } = useSettings();
  return (
    <Panel icon={ShieldIcon} title={t("first_steps.access.panel")} lead={t("first_steps.access.panel_lead")}>
      <Setting label={t("first_steps.access.address")} why={t("first_steps.access.address_why")}>
        <code className="first-steps-address">{window.location.origin}</code>
      </Setting>
      <p className="panel-say">{t("first_steps.access.outside")}</p>
    </Panel>
  );
}
