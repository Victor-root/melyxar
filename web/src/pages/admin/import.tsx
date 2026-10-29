/*
 * Taking up what an Emby or Jellyfin server kept of each account: its data
 * folder named on the server, a preview of what would be taken up, the
 * account each account there becomes here, and the collections kept, then
 * what was taken up.
 *
 * The oldest versions read are said before anything is asked, and again in
 * the refusal of a backup too old. Nothing is written before the last button,
 * and that one asks first.
 */

import { useState } from "react";
import { api } from "../../api";
import { ApiError } from "../../api";
import type { ImportKind, ImportPreview, ImportReport, ManagedAccount } from "../../api";
import { refusalOf, useAsked } from "../../asking";
import { Modal } from "../../components/modal";
import { PageHead, Panel, Picker, Setting, Stat, StatePill, Toggle } from "../../components/panel";
import {
  AccountIcon,
  CollectionIcon,
  DownloadIcon,
  EyeIcon,
  HeartIcon,
  MusicIcon,
  PlaybackIcon,
  PlaylistIcon,
  WarningIcon,
} from "../../icons";
import type { IconProps } from "../../icons";
import { useMarkActions } from "../../marks";
import { useMusicMarks } from "../../music/marks";
import { useSettings } from "../../settings";
import { choicesOf, firstCollections, firstPlans, readyToTakeUp } from "./import-choices";
import type { AccountPlan } from "./import-choices";

/** What a refusal of the server said, worded. */
function refusalWording(error: unknown, t: ReturnType<typeof useSettings>["t"]): string {
  const reason = refusalOf(error);
  const details = error instanceof ApiError ? (error.details ?? {}) : {};
  switch (reason) {
    case "too_old":
      return t("import.refused.too_old", {
        server: t(`import.server.${String(details.server)}`),
        oldest: String(details.oldest),
      });
    case "account":
      return t("import.refused.account", {
        name: String(details.name),
        why: t(`refused.account.${String(details.why)}`, {
          shortest: Number(details.shortest),
          longest: Number(details.longest),
        }),
      });
    case "unreadable":
      return t("import.refused.unreadable", { why: String(details.why) });
    case "path_needed":
    case "not_found":
    case "not_a_server":
      return t(`import.refused.${reason}`);
    default:
      return t("refused.generic");
  }
}

export function AdminImport() {
  const { t } = useSettings();
  const [path, setPath] = useState("");
  const [read, setRead] = useState<{ path: string; preview: ImportPreview } | null>(null);
  const [plans, setPlans] = useState<Record<string, AccountPlan>>({});
  const [collections, setCollections] = useState<Set<string>>(new Set());
  const [busy, setBusy] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const [asking, setAsking] = useState(false);
  const [report, setReport] = useState<ImportReport | null>(null);
  const here = useAsked((signal) => api.accounts(signal), [report]);
  const { rowsHaveMoved } = useMarkActions();
  const { playlistsHaveMoved } = useMusicMarks();

  const readIt = async () => {
    setBusy(true);
    setRefused(null);
    setReport(null);
    try {
      const preview = await api.importPreview(path.trim());
      setRead({ path: path.trim(), preview });
      setPlans(firstPlans(preview));
      setCollections(firstCollections(preview));
    } catch (error) {
      setRead(null);
      setRefused(refusalWording(error, t));
    } finally {
      setBusy(false);
    }
  };

  const takeUp = async () => {
    if (!read) {
      return;
    }
    setAsking(false);
    setBusy(true);
    setRefused(null);
    try {
      setReport(await api.importTakeUp(read.path, choicesOf(plans, collections)));
      setRead(null);
      // Every screen showing what was watched, liked or listened to reads it
      // again: it changed for whole accounts at once.
      rowsHaveMoved();
      playlistsHaveMoved();
    } catch (error) {
      setRefused(refusalWording(error, t));
    } finally {
      setBusy(false);
    }
  };

  return (
    <>
      <PageHead lead={t("import.lead")} />
      <div className="panels import-panels">
        <Panel icon={DownloadIcon} title={t("import.title")}>
          <ul className="import-takes">
            <li>{t("import.takes.accounts")}</li>
            <li>{t("import.takes.watched")}</li>
            <li>{t("import.takes.favourites")}</li>
            <li>{t("import.takes.music")}</li>
            <li>{t("import.takes.lists")}</li>
          </ul>
          <p className="panel-notice">{t("import.no_passwords")}</p>
          <Setting label={t("import.versions")} why={t("import.versions_why")}>
            <span className="import-versions">
              <StatePill state="ok">Emby 4.8</StatePill>
              <StatePill state="ok">Jellyfin 10.9.11</StatePill>
            </span>
          </Setting>
          <form
            className="import-from"
            onSubmit={(event) => {
              event.preventDefault();
              void readIt();
            }}
          >
            <label className="import-from-label" htmlFor="import-path">
              {t("import.path")}
            </label>
            <input
              id="import-path"
              type="text"
              className="field-line"
              placeholder="/var/lib/emby"
              spellCheck={false}
              autoComplete="off"
              value={path}
              onChange={(event) => setPath(event.target.value)}
            />
            <button type="submit" className="button button-small button-accent" disabled={busy || path.trim() === ""}>
              {t("import.read")}
            </button>
            <span className="setting-why">{t("import.path_why")}</span>
          </form>
          {refused && <p className="panel-notice panel-notice-trouble">{refused}</p>}
          {report && <Taken report={report} />}
        </Panel>

        {read && (
          <Preview
            preview={read.preview}
            here={here.answer ?? []}
            plans={plans}
            setPlans={setPlans}
            collections={collections}
            setCollections={setCollections}
            busy={busy}
            onTakeUp={() => setAsking(true)}
          />
        )}
      </div>

      {asking && (
        <Modal
          title={t("import.ask_title")}
          onClose={() => setAsking(false)}
          footer={
            <>
              <button type="button" className="button button-quiet" onClick={() => setAsking(false)}>
                {t("lists.cancel")}
              </button>
              <button type="button" className="button button-accent" onClick={() => void takeUp()}>
                {t("import.take_up")}
              </button>
            </>
          }
        >
          <p>{t("import.ask_why")}</p>
        </Modal>
      )}
    </>
  );
}

/** The heart of a favourite, as a picture that is always filled. */
function FavouriteIcon(props: IconProps) {
  return <HeartIcon filled {...props} />;
}

const KIND_ICONS: Record<ImportKind, typeof PlaybackIcon> = {
  movie: PlaybackIcon,
  series: PlaybackIcon,
  season: PlaybackIcon,
  episode: PlaybackIcon,
  video: PlaybackIcon,
  song: MusicIcon,
  album: MusicIcon,
  artist: MusicIcon,
};

function Preview({
  preview,
  here,
  plans,
  setPlans,
  collections,
  setCollections,
  busy,
  onTakeUp,
}: {
  preview: ImportPreview;
  here: ManagedAccount[];
  plans: Record<string, AccountPlan>;
  setPlans: (plans: Record<string, AccountPlan>) => void;
  collections: Set<string>;
  setCollections: (collections: Set<string>) => void;
  busy: boolean;
  onTakeUp: () => void;
}) {
  const { t } = useSettings();
  const plan = (key: string, changed: Partial<AccountPlan>) =>
    setPlans({ ...plans, [key]: { ...plans[key], ...changed } });
  const options = [
    ...here.map((account) => [`existing:${account.id}`, t("import.becomes_existing", { name: account.name })] as const),
    ["new", t("import.becomes_new")] as const,
    ["nothing", t("import.becomes_nothing")] as const,
  ];
  const valueOf = (one: AccountPlan) => (one.becomes === "existing" ? `existing:${one.user}` : one.becomes);

  return (
    <>
      <Panel
        icon={EyeIcon}
        title={t("import.found_title")}
        lead={t("import.found_lead")}
        action={<StatePill state="ok">{t(`import.server.${preview.server}`)}</StatePill>}
      >
        <div className="stats">
          {preview.found.map((found) => (
            <Stat
              key={found.kind}
              icon={KIND_ICONS[found.kind]}
              label={t(`import.kind.${found.kind}`)}
              value={t("import.found_of", { found: found.found, total: found.total })}
              state={found.found === found.total ? "ok" : "attention"}
            />
          ))}
          <Stat icon={PlaybackIcon} label={t("import.watched")} value={preview.taken.watched} />
          <Stat icon={PlaybackIcon} label={t("import.left_partway")} value={preview.taken.left_partway} />
          <Stat icon={FavouriteIcon} label={t("import.favourites")} value={preview.taken.favourites} />
          <Stat icon={MusicIcon} label={t("import.listens")} value={preview.taken.listens} />
        </div>
        {preview.lost_count > 0 && (
          <details className="import-lost">
            <summary>
              <WarningIcon size={15} />
              {t("import.lost", { count: preview.lost_count })}
            </summary>
            <p className="setting-why">{t("import.lost_why")}</p>
            <ul>
              {preview.lost.map((lost, index) => (
                <li key={index}>
                  <span className="import-lost-name">
                    {lost.name}
                    {lost.year ? ` (${lost.year})` : ""}
                  </span>
                  <span className="import-lost-kind">{t(`import.kind.${lost.kind}`)}</span>
                  {lost.path && <span className="import-lost-path">{lost.path}</span>}
                </li>
              ))}
            </ul>
          </details>
        )}
      </Panel>

      <Panel icon={AccountIcon} title={t("import.accounts")} lead={t("import.accounts_lead")}>
        {preview.accounts.map((account) => {
          const one = plans[account.key];
          return (
            <div key={account.key} className="import-account">
              <span className="import-account-name">
                {account.name}
                {account.administrator && <StatePill state="ok">{t("import.administrator")}</StatePill>}
                {account.disabled && <StatePill state="attention">{t("import.disabled")}</StatePill>}
              </span>
              <Picker
                value={valueOf(one)}
                options={options}
                label={t("import.becomes", { name: account.name })}
                onPick={(picked) =>
                  picked.startsWith("existing:")
                    ? plan(account.key, { becomes: "existing", user: picked.slice("existing:".length) })
                    : plan(account.key, { becomes: picked as "new" | "nothing" })
                }
              />
              {one.becomes === "new" && (
                <span className="import-account-new">
                  <input
                    type="text"
                    className="field-line"
                    aria-label={t("import.new_name")}
                    placeholder={t("import.new_name")}
                    autoComplete="off"
                    value={one.name}
                    onChange={(event) => plan(account.key, { name: event.target.value })}
                  />
                  <input
                    type="password"
                    className="field-line"
                    aria-label={t("import.new_password")}
                    placeholder={t("import.new_password")}
                    autoComplete="new-password"
                    value={one.password}
                    onChange={(event) => plan(account.key, { password: event.target.value })}
                  />
                </span>
              )}
            </div>
          );
        })}
      </Panel>

      {(preview.playlists.length > 0 || preview.collections.length > 0) && (
        <Panel icon={PlaylistIcon} title={t("import.lists")} lead={t("import.lists_lead")}>
          {preview.playlists.map((playlist, index) => (
            <Setting
              key={`p${index}`}
              label={playlist.name}
              why={t("import.playlist_of", {
                owner: playlist.owner ?? t("import.everybody"),
                found: playlist.found,
                total: playlist.items,
              })}
            >
              <PlaylistIcon size={18} />
            </Setting>
          ))}
          {preview.collections.map((collection) => (
            <Setting
              key={`c${collection.name}`}
              label={collection.name}
              why={t(collection.automatic ? "import.collection_automatic" : "import.collection_of", {
                found: collection.found,
                total: collection.items,
              })}
            >
              <span className="import-collection">
                <CollectionIcon size={18} />
                <Toggle
                  label={t("import.take_collection", { name: collection.name })}
                  checked={collections.has(collection.name)}
                  onChange={(on) => {
                    const next = new Set(collections);
                    if (on) {
                      next.add(collection.name);
                    } else {
                      next.delete(collection.name);
                    }
                    setCollections(next);
                  }}
                />
              </span>
            </Setting>
          ))}
        </Panel>
      )}

      <div className="import-go">
        <span className="setting-why">{t("import.kept_why")}</span>
        <button
          type="button"
          className="button button-accent"
          disabled={busy || !readyToTakeUp(plans)}
          onClick={onTakeUp}
        >
          <DownloadIcon size={16} />
          {t("import.take_up")}
        </button>
      </div>
    </>
  );
}

/** What was taken up, said once it is done. */
function Taken({ report }: { report: ImportReport }) {
  const { t } = useSettings();
  return (
    <div className="panel-notice panel-notice-ok import-taken">
      <p>
        {t("import.done", {
          made: report.accounts_made,
          joined: report.accounts_joined,
          watched: report.taken.watched,
          partway: report.taken.left_partway,
          favourites: report.taken.favourites,
          listens: report.taken.listens,
          playlists: report.playlists,
          collections: report.collections,
        })}
      </p>
      {report.left_alone.length > 0 && <p>{t("import.left_alone", { names: report.left_alone.join(", ") })}</p>}
    </div>
  );
}
