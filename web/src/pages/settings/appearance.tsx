/*
 * How the interface looks and what language it speaks, and which buttons the
 * bar at the top keeps.
 *
 * Kept by the account rather than by this browser, so a choice made on one
 * machine is the same choice on the next.
 */

import type { Backdrop, HeaderButton } from "../../api";
import { LanguagePicker } from "../../components/language-picker";
import { PageHead, Panel, Picker, Setting, Toggle } from "../../components/panel";
import { Sortable } from "../../components/sortable";
import { useAccount } from "../../account";
import { offeredButtons } from "../../buttons";
import { LIGHTS } from "../../lights";
import {
  BellIcon,
  ClockIcon,
  CollectionIcon,
  PlaylistIcon,
  DashboardIcon,
  GearIcon,
  HeartIcon,
  PaletteIcon,
  RefreshIcon,
  RequestIcon,
  ResetIcon,
  ScreenCastIcon,
  SearchIcon,
  SlidersIcon,
  TickIcon,
} from "../../icons";
import { useRequests } from "../../requests/store";
import { OFFERED_ACCENTS, useSettings } from "../../settings";
import { reorderedAmong } from "../../sorting";
import type { ThemeChoice } from "../../settings";

export function MyAppearance() {
  const { t, theme, setTheme, accent, setAccent } = useSettings();

  return (
    <>
      <PageHead lead={t("settings.appearance_why")} />

      <Panel icon={PaletteIcon} title={t("settings.appearance")}>
        <Setting label={t("nav.theme")} why={t("me.theme_why")}>
          <Picker<ThemeChoice>
            label={t("nav.theme")}
            value={theme}
            onPick={setTheme}
            options={[
              ["system", t("theme.system")],
              ["dark", t("theme.dark")],
              ["light", t("theme.light")],
            ]}
          />
        </Setting>
        <Setting label={t("nav.language")}>
          <LanguagePicker />
        </Setting>
        {/* A handful at a press, and any other by hand: the colour is what
            every active and pressable thing in the interface wears. */}
        <Setting label={t("me.accent")} why={t("me.accent_why")}>
          <div className="swatches" role="radiogroup" aria-label={t("me.accent")}>
            {OFFERED_ACCENTS.map((colour) => {
              const chosen = colour === accent;
              return (
                <button
                  key={colour}
                  type="button"
                  role="radio"
                  aria-checked={chosen}
                  aria-label={colour}
                  className={`swatch${chosen ? " swatch-on" : ""}`}
                  style={{ ["--swatch" as string]: colour }}
                  onClick={() => setAccent(colour)}
                >
                  {chosen && <TickIcon size={14} />}
                </button>
              );
            })}
            <label
              className={`swatch swatch-own${OFFERED_ACCENTS.includes(accent) ? "" : " swatch-on"}`}
              title={t("me.accent_own")}
              style={{ ["--swatch" as string]: accent }}
            >
              <input
                type="color"
                value={accent}
                aria-label={t("me.accent_own")}
                onChange={(event) => setAccent(event.target.value)}
              />
            </label>
          </div>
        </Setting>
        <BackdropChoice />
      </Panel>

      <TopBar />
    </>
  );
}

/**
 * What is drawn behind the pages: nothing, or light, and then which of the
 * paintings of light, each shown as it would be, shrunk into a tile and in
 * the accent in force.
 */
function BackdropChoice() {
  const { t, backdrop, setBackdrop, backdropLight, setBackdropLight } = useSettings();

  return (
    <>
      <Setting label={t("settings.backdrop")} why={t("settings.backdrop_why")}>
        <Picker<Backdrop>
          label={t("settings.backdrop")}
          value={backdrop}
          onPick={setBackdrop}
          options={[
            ["light", t("backdrop.light")],
            ["library", t("backdrop.library")],
            ["none", t("backdrop.none")],
          ]}
        />
      </Setting>
      {backdrop === "light" && (
        <div className="lights" role="radiogroup" aria-label={t("settings.backdrop_light")}>
          {LIGHTS.map((light) => {
            const chosen = light === backdropLight;
            return (
              <button
                key={light}
                type="button"
                role="radio"
                aria-checked={chosen}
                className={`light-choice${chosen ? " light-choice-on" : ""}`}
                onClick={() => setBackdropLight(light)}
              >
                <span className="light-tile drift" data-light={light} aria-hidden="true" />
                <span className="light-name">{t(`backdrop.light.${light}`)}</span>
              </button>
            );
          })}
        </div>
      )}
    </>
  );
}

const BUTTON_NAMES: Record<HeaderButton, string> = {
  search: "nav.search",
  favourites: "nav.favourites",
  watch_later: "nav.watch_later",
  collections: "nav.collections",
  playlists: "nav.playlists",
  requests: "requests.title",
  notifications: "nav.notifications",
  scan: "home.scan",
  administration: "nav.administration",
  cast: "nav.cast",
  settings: "nav.settings",
};

/**
 * Everything about the bar at the top in one place: whether it steps aside
 * while a page is read down, and its buttons, in which order and which of
 * them stay on it. A hidden one moves into the account's menu and keeps its
 * place in the list, so shown again it comes back where it was.
 */
function TopBar() {
  const {
    t,
    headerHides,
    setHeaderHides,
    headerButtons,
    setHeaderButtons,
    buttonsInTheBar,
    setButtonsInTheBar,
    resetHeaderButtons,
  } = useSettings();
  const { account } = useAccount();
  const name = (button: HeaderButton) => t(BUTTON_NAMES[button]);
  const { access } = useRequests();
  const offered = offeredButtons(headerButtons, {
    administrator: account?.is_administrator === true,
    mayRequest: access?.may_ask === true,
  });

  return (
    <Panel
      icon={SlidersIcon}
      title={t("settings.header")}
      lead={t("settings.header_buttons_why")}
      action={
        <button type="button" className="button button-small" onClick={resetHeaderButtons}>
          <ResetIcon size={14} />
          {t("settings.header_buttons_reset")}
        </button>
      }
    >
      <Setting label={t("settings.header_hides")} why={t("settings.header_hides_why")}>
        <Toggle label={t("settings.header_hides")} checked={headerHides} onChange={setHeaderHides} />
      </Setting>
      <Sortable
        items={offered}
        keyOf={(button) => button}
        nameOf={name}
        onMove={(reordered) => setHeaderButtons(reorderedAmong(headerButtons, offered, reordered))}
        lineClass={(button) => (buttonsInTheBar.includes(button) ? undefined : "order-line-off")}
      >
        {(button) => (
          <>
            <span className="line-mark" aria-hidden="true">
              <ButtonMark button={button} />
            </span>
            <span className="order-name">{name(button)}</span>
            <Toggle
              label={t("settings.header_button_shown", { name: name(button) })}
              checked={buttonsInTheBar.includes(button)}
              onChange={(shown) =>
                setButtonsInTheBar(
                  shown
                    ? [...buttonsInTheBar, button]
                    : buttonsInTheBar.filter((one) => one !== button),
                )
              }
            />
          </>
        )}
      </Sortable>
    </Panel>
  );
}

/** The icon each button wears on the bar, so a line is found by eye. */
function ButtonMark({ button }: { button: HeaderButton }) {
  switch (button) {
    case "search":
      return <SearchIcon size={18} />;
    case "notifications":
      return <BellIcon size={18} />;
    case "favourites":
      return <HeartIcon size={18} filled={false} />;
    case "watch_later":
      return <ClockIcon size={18} />;
    case "collections":
      return <CollectionIcon size={18} />;
    case "playlists":
      return <PlaylistIcon size={18} />;
    case "requests":
      return <RequestIcon size={18} />;
    case "scan":
      return <RefreshIcon size={18} />;
    case "administration":
      return <DashboardIcon size={18} />;
    case "cast":
      return <ScreenCastIcon size={18} />;
    case "settings":
      return <GearIcon size={18} />;
  }
}
