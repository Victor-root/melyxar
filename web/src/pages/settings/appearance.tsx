/*
 * How the interface looks and what language it speaks, and which buttons the
 * bar at the top keeps.
 *
 * Kept by the account rather than by this browser, so a choice made on one
 * machine is the same choice on the next.
 */

import type { HeaderButton } from "../../api";
import { PageHead, Panel, Picker, Setting, Toggle } from "../../components/panel";
import { Sortable } from "../../components/sortable";
import {
  BellIcon,
  ClockIcon,
  HeartIcon,
  PaletteIcon,
  SearchIcon,
  SlidersIcon,
  TickIcon,
} from "../../icons";
import { OFFERED_ACCENTS, useSettings } from "../../settings";
import type { ThemeChoice } from "../../settings";

export function MyAppearance() {
  const { t, language, setLanguage, theme, setTheme, accent, setAccent, headerHides, setHeaderHides } =
    useSettings();

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
          <Picker
            label={t("nav.language")}
            value={language}
            onPick={(picked) => setLanguage(picked === "fr" ? "fr" : "en")}
            options={[
              ["en", "English"],
              ["fr", "Français"],
            ]}
          />
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
        <Setting label={t("settings.header_hides")} why={t("settings.header_hides_why")}>
          <Toggle label={t("settings.header_hides")} checked={headerHides} onChange={setHeaderHides} />
        </Setting>
      </Panel>

      <HeaderButtons />
    </>
  );
}

const BUTTON_NAMES: Record<HeaderButton, string> = {
  search: "nav.search",
  notifications: "nav.notifications",
  favourites: "nav.favourites",
  watch_later: "nav.watch_later",
};

/**
 * The buttons at the right end of the bar: in which order, and which of them
 * stay on it. A hidden one moves into the account's menu and keeps its place
 * in the list, so shown again it comes back where it was.
 */
function HeaderButtons() {
  const { t, headerButtons, setHeaderButtons, buttonsInTheMenu, setButtonsInTheMenu } =
    useSettings();
  const name = (button: HeaderButton) => t(BUTTON_NAMES[button]);

  return (
    <Panel icon={SlidersIcon} title={t("settings.header_buttons")} lead={t("settings.header_buttons_why")}>
      <Sortable
        items={headerButtons}
        keyOf={(button) => button}
        nameOf={name}
        onMove={setHeaderButtons}
        lineClass={(button) => (buttonsInTheMenu.includes(button) ? "order-line-off" : undefined)}
      >
        {(button) => (
          <>
            <span className="line-mark" aria-hidden="true">
              <ButtonMark button={button} />
            </span>
            <span className="order-name">{name(button)}</span>
            <Toggle
              label={t("settings.header_button_shown", { name: name(button) })}
              checked={!buttonsInTheMenu.includes(button)}
              onChange={(shown) =>
                setButtonsInTheMenu(
                  shown
                    ? buttonsInTheMenu.filter((one) => one !== button)
                    : [...buttonsInTheMenu, button],
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
  }
}
