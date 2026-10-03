import { Link, NavLink } from "react-router-dom";
import { BellIcon, TickIcon } from "../icons";
import { Dropdown } from "../components/dropdown";
import { sayPoint } from "../pages/admin/activity";
import { useSettings } from "../settings";
import { useAttention } from "./attention";

/**
 * What deserves a look, for an administrator: how many points, the worst of
 * them in its colour, and the list itself on a press.
 *
 * For anybody else there is nothing behind it yet, and it stays greyed and
 * saying so rather than left out: a function nobody can see is a function
 * nobody knows is coming.
 */
export function Bell({ administrator }: { administrator: boolean }) {
  const { t, language } = useSettings();
  const { points, markSeen } = useAttention();

  if (!administrator) {
    return (
      <button
        type="button"
        className="header-icon"
        disabled
        title={t("nav.later")}
        aria-label={`${t("nav.notifications")} (${t("nav.later")})`}
      >
        <BellIcon size={24} />
      </button>
    );
  }

  const shown = points ?? [];
  const worst = shown.some((point) => point.state === "trouble") ? "trouble" : "attention";
  return (
    <Dropdown
      className="header-bell"
      icon={t("admin.watch")}
      listClassName="bell-list"
      reachable
      label={
        <>
          <BellIcon size={24} />
          {shown.length > 0 && (
            <span className={`bell-count bell-count-${worst}`}>{shown.length}</span>
          )}
        </>
      }
    >
      {/* Marking them seen sits with the title rather than under the last
          point, where a long list would push it out of reach. */}
      <span className="bell-head">
        <span className="bell-title">{t("admin.watch")}</span>
        {shown.some((point) => point.may_be_seen) && (
          <button type="button" className="bell-seen" onClick={() => void markSeen()}>
            <TickIcon size={14} />
            {t("attention.mark_seen")}
          </button>
        )}
      </span>
      {shown.length === 0 && <span className="bell-none">{t("attention.none")}</span>}
      {shown.map((point, index) => {
        const said = sayPoint(point, t, language);
        return (
          <Link key={index} to={said.to} className="header-menu-line bell-point">
            <span className={`state-dot state-${point.state}`} aria-hidden="true" />
            <span>{said.title}</span>
          </Link>
        );
      })}
    </Dropdown>
  );
}

/**
 * The bell moved into the account's menu: for an administrator, the page
 * where the points are listed, with how many there are; greyed for anybody
 * else, as the bell is.
 */
export function BellLine({ administrator }: { administrator: boolean }) {
  const { t } = useSettings();
  const { points } = useAttention();

  if (!administrator) {
    return (
      <span className="header-menu-line header-menu-later" aria-disabled="true" title={t("nav.later")}>
        <BellIcon size={16} />
        {t("nav.notifications")}
      </span>
    );
  }
  const shown = points ?? [];
  const worst = shown.some((point) => point.state === "trouble") ? "trouble" : "attention";
  return (
    <NavLink to="/admin" end className="header-menu-line">
      <BellIcon size={16} />
      {t("admin.watch")}
      {shown.length > 0 && (
        <span className={`bell-count bell-count-${worst} bell-count-line`}>{shown.length}</span>
      )}
    </NavLink>
  );
}
