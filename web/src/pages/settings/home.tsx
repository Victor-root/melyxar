/*
 * How the home page opens: its banner, which of its sections it shows and in
 * what order, and the order the kinds of library are laid out in. Both orders
 * are dragged into place.
 *
 * The banner's two numbers apply as they are dragged, on every page, because
 * they are written onto the document rather than passed down. The share of the
 * picture kept is said in words, because it is what the height really decides
 * and it cannot be seen from a slider.
 */

import type { HomeSection, LibraryKind } from "../../api";
import { PageHead, Panel, Setting, Slider, Toggle } from "../../components/panel";
import { Sortable } from "../../components/sortable";
import {
  BinocularsIcon,
  CameraIcon,
  CollectionIcon,
  EyeIcon,
  HomeIcon,
  ImageIcon,
  KindIcon,
  SlidersIcon,
} from "../../icons";
import { kindsOnTheHomePage, nameOfKind, newestOfKind, useLibraries } from "../../libraries";
import { useMarks } from "../../marks";
import { kindOfSection, sectionsOnOffer } from "../../screens/home";
import { usePreferences } from "../../screens/settings";
import type { Preferences } from "../../screens/settings";
import { useSettings } from "../../settings";
import { reorderedAmong } from "../../sorting";
import { shareOfThePictureKept } from "./banner";

export function MyHomePage() {
  const { t } = useSettings();
  const preferences = usePreferences();
  return (
    <>
      <PageHead lead={t("me.home_lead")} />
      <Banner preferences={preferences} />
      <HomeSections preferences={preferences} />
      <LibraryOrder preferences={preferences} />
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
            // The banner is part of the page this changes, so the page is
            // read again once the server holds it, rather than left showing
            // the old handful.
            onChange={(banner_at_random) => {
              void change({ banner_at_random }).then(marks.rowsHaveMoved);
            }}
          />
        </Setting>
      )}
    </Panel>
  );
}

/**
 * The sections of the home page below its banner: in which order, and which
 * of them it shows. The newest of each kind of library is a section of its
 * own, offered only for the kinds this account holds. A hidden section keeps
 * its place in the list, so shown again it comes back where it was.
 */
function HomeSections({ preferences }: { preferences: Preferences }) {
  const { t } = useSettings();
  const marks = useMarks();
  const libraries = useLibraries();
  const { kept, change } = preferences;

  if (!kept) {
    return null;
  }
  const hidden = kept.hidden_home_sections;
  const offered = sectionsOnOffer(kept.home_sections, kindsOnTheHomePage(kept.home_order, libraries.all));
  const name = (section: HomeSection) => {
    const kind = kindOfSection(section);
    return kind ? newestOfKind(kind, libraries.all, t) : t(`home_section.${section}`);
  };

  // The home page is read again once the server holds the change, so it is
  // already right when somebody goes back to it.
  const show = (section: HomeSection, shown: boolean) =>
    change({
      hidden_home_sections: shown
        ? hidden.filter((one) => one !== section)
        : [...hidden, section],
    }).then(marks.rowsHaveMoved);

  return (
    <Panel icon={SlidersIcon} title={t("settings.home_sections")} lead={t("settings.home_sections_why")}>
      <Sortable
        items={offered}
        keyOf={(section) => section}
        nameOf={name}
        onMove={(reordered) =>
          change({ home_sections: reorderedAmong(kept.home_sections, offered, reordered) }).then(
            marks.rowsHaveMoved,
          )
        }
        lineClass={(section) => (hidden.includes(section) ? "order-line-off" : undefined)}
      >
        {(section) => (
          <>
            <SectionMark section={section} />
            <span className="order-name">{name(section)}</span>
            <Toggle
              label={t("settings.home_section_shown", { name: name(section) })}
              checked={!hidden.includes(section)}
              onChange={(shown) => show(section, shown)}
            />
          </>
        )}
      </Sortable>
    </Panel>
  );
}

/** What each section is drawn with on the home page, so a line of the list
 *  is found by eye as well as by its name. */
function SectionMark({ section }: { section: HomeSection }) {
  const kind = kindOfSection(section);
  return (
    <span className="line-mark" aria-hidden="true">
      {kind ? (
        <KindIcon kind={kind} size={18} />
      ) : section === "band" ? (
        <CollectionIcon size={18} />
      ) : section === "carry_on" ? (
        <EyeIcon size={18} />
      ) : section === "up_next" ? (
        <BinocularsIcon size={18} />
      ) : (
        <CameraIcon size={18} />
      )}
    </span>
  );
}

/**
 * The order of the tiles leading to each kind of library. Only the kinds
 * this account holds are offered; the rows of each kind are placed in the
 * order of the home page above.
 */
function LibraryOrder({ preferences }: { preferences: Preferences }) {
  const { t } = useSettings();
  const marks = useMarks();
  const libraries = useLibraries();
  const { kept, change } = preferences;

  if (!kept) {
    return null;
  }
  const shown = kindsOnTheHomePage(kept.home_order, libraries.all);

  return (
    <Panel icon={HomeIcon} title={t("settings.library_order")} lead={t("settings.library_order_why")}>
      {shown.length < 2 ? (
        <p className="empty-line">{t("me.library_order_single")}</p>
      ) : (
        <Sortable
          items={shown}
          keyOf={(kind) => kind}
          nameOf={(kind) => nameOfKind(kind, libraries.all, t)}
          onMove={(reordered: LibraryKind[]) =>
            change({ home_order: reorderedAmong(kept.home_order, shown, reordered) }).then(
              marks.rowsHaveMoved,
            )
          }
        >
          {(kind) => (
            <>
              <span className="line-mark" aria-hidden="true">
                <KindIcon kind={kind} size={18} />
              </span>
              <span className="order-name">{nameOfKind(kind, libraries.all, t)}</span>
            </>
          )}
        </Sortable>
      )}
    </Panel>
  );
}
