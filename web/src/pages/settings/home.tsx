/*
 * How the home page opens: its banner, and the order the kinds of library
 * are laid out in.
 *
 * The banner's two numbers apply as they are dragged, on every page, because
 * they are written onto the document rather than passed down. The share of the
 * picture kept is said in words, because it is what the height really decides
 * and it cannot be seen from a slider.
 */

import type { LibraryKind } from "../../api";
import { PageHead, Panel, Setting, Slider, Toggle } from "../../components/panel";
import { ChevronDownIcon, ChevronUpIcon, HomeIcon, ImageIcon, KindIcon } from "../../icons";
import { kindsOnTheHomePage, movedOnTheHomePage, useLibraries } from "../../libraries";
import { useMarks } from "../../marks";
import { usePreferences } from "../../screens/settings";
import type { Preferences } from "../../screens/settings";
import { useSettings } from "../../settings";
import { shareOfThePictureKept } from "./banner";

export function MyHomePage() {
  const { t } = useSettings();
  const preferences = usePreferences();
  return (
    <>
      <PageHead lead={t("me.home_lead")} />
      <Banner preferences={preferences} />
      <HomeOrder preferences={preferences} />
    </>
  );
}

function Banner({ preferences }: { preferences: Preferences }) {
  const {
    t,
    bannerHeight,
    setBannerHeight,
    bannerCut,
    setBannerCut,
    bannerFillsTheScreen,
    setBannerFillsTheScreen,
    bannerShown,
    setBannerShown,
  } = useSettings();
  const marks = useMarks();
  const { kept, change } = preferences;

  return (
    <Panel icon={ImageIcon} title={t("settings.banner")} lead={t("settings.banner_why")}>
      <Setting label={t("settings.banner_shown")}>
        <Toggle
          label={t("settings.banner_shown")}
          checked={bannerShown}
          onChange={(shown) => {
            setBannerShown(shown);
            // The server leaves the banner out of the page it sends, so the
            // page is read again to lose it or to get it back.
            marks.rowsHaveMoved();
          }}
        />
      </Setting>
      <Setting label={t("settings.banner_height")}>
        <Slider
          label={t("settings.banner_height")}
          min={kept?.banner_height_range[0] ?? 0.2}
          max={kept?.banner_height_range[1] ?? 0.55}
          step={0.01}
          value={bannerHeight}
          shown={
            bannerFillsTheScreen
              ? t("settings.banner_whole_window")
              : t("settings.banner_kept", { percent: shareOfThePictureKept(bannerHeight) })
          }
          /* Nothing left for it to decide while the banner takes the whole
             window, and still readable, so what it is set to can be seen
             before it is turned back on. */
          disabled={!bannerShown || bannerFillsTheScreen}
          onChange={setBannerHeight}
        />
      </Setting>
      <Setting label={t("settings.banner_cut")} why={t("settings.banner_cut_why")}>
        <Slider
          label={t("settings.banner_cut")}
          min={0}
          max={1}
          step={0.01}
          value={bannerCut}
          shown={`${Math.round(bannerCut * 100)} %`}
          disabled={!bannerShown}
          onChange={setBannerCut}
        />
      </Setting>
      <Setting label={t("settings.banner_whole")} why={t("settings.banner_whole_why")}>
        <Toggle
          label={t("settings.banner_whole")}
          checked={bannerFillsTheScreen}
          disabled={!bannerShown}
          onChange={setBannerFillsTheScreen}
        />
      </Setting>
      {kept && (
        <Setting label={t("settings.banner_at_random")} why={t("settings.banner_at_random_why")}>
          <Toggle
            label={t("settings.banner_at_random")}
            checked={kept.banner_at_random}
            disabled={!bannerShown}
            onChange={(banner_at_random) => {
              void change({ banner_at_random });
              // The banner is part of the page this changes, so the page is
              // read again rather than left showing the old handful.
              marks.rowsHaveMoved();
            }}
          />
        </Setting>
      )}
    </Panel>
  );
}

/**
 * The order the home page lays the kinds of library out in, its tiles and its
 * rows alike. Only the kinds this account holds are offered.
 */
function HomeOrder({ preferences }: { preferences: Preferences }) {
  const { t } = useSettings();
  const marks = useMarks();
  const libraries = useLibraries();
  const { kept, change } = preferences;

  if (!kept) {
    return null;
  }
  const shown = kindsOnTheHomePage(kept.home_order, libraries.all);

  // The home page is read again once the server holds the new order, so it is
  // already right when somebody goes back to it.
  const move = (kind: LibraryKind, step: -1 | 1) =>
    change({ home_order: movedOnTheHomePage(kept.home_order, shown, kind, step) }).then(
      marks.rowsHaveMoved,
    );

  return (
    <Panel icon={HomeIcon} title={t("settings.home_order")} lead={t("settings.home_order_why")}>
      {shown.length < 2 ? (
        <p className="empty-line">{t("me.home_order_single")}</p>
      ) : (
        <ol className="order">
          {shown.map((kind, place) => {
            const name = t(`kind.${kind}`);
            return (
              <li key={kind} className="order-line">
                <span className="order-place">{place + 1}</span>
                <span className="line-mark" aria-hidden="true">
                  <KindIcon kind={kind} size={18} />
                </span>
                <span className="order-name">{name}</span>
                <button
                  className="button button-small button-quiet"
                  onClick={() => move(kind, -1)}
                  disabled={place === 0}
                  aria-label={t("settings.home_order_up", { kind: name })}
                  title={t("settings.home_order_up", { kind: name })}
                >
                  <ChevronUpIcon size={16} />
                </button>
                <button
                  className="button button-small button-quiet"
                  onClick={() => move(kind, 1)}
                  disabled={place === shown.length - 1}
                  aria-label={t("settings.home_order_down", { kind: name })}
                  title={t("settings.home_order_down", { kind: name })}
                >
                  <ChevronDownIcon size={16} />
                </button>
              </li>
            );
          })}
        </ol>
      )}
    </Panel>
  );
}
