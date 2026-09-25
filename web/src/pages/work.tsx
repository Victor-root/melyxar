/*
 * The page of one work.
 *
 * Its top is one screen and never more: what it is, which copy and which
 * tracks it will start with, the buttons, how far in it already is, what it
 * is about and who made it. The synopsis is what gives way when the screen is
 * short, its column let out first and its text cut last, on a whole line and
 * only then with the word that opens the rest. Everything else is rows under
 * it, the same rows as the home page: who is in it, its chapters, what is
 * like it, and at the very end what the files hold.
 */

import { useEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import type { Card as CardData, Child, PlaybackPlan, Version, Work } from "../api";
import { useAccount } from "../account";
import { useTold } from "../asking";
import { WayBackUp } from "../components/ancestry";
import { Card } from "../components/card";
import { useWorkMenu } from "../components/cardmenu";
import { Panel, Picker } from "../components/panel";
import { PersonCard } from "../components/person";
import { TrailerDialog } from "../components/trailer";
import { Row, RowHead } from "../components/row";
import { useFittedText } from "../fitting";
import {
  CollectionIcon,
  FilmIcon,
  FolderIcon,
  HeartIcon,
  MoreIcon,
  PeopleIcon,
  PlaylistIcon,
  SoundIcon,
  StarIcon,
  SubtitlesIcon,
  TagIcon,
  TickIcon,
  TrailerIcon,
} from "../icons";
import { languageName } from "../languages";
import { useMarks } from "../marks";
import {
  containerName,
  howLong,
  howMany,
  nameOfOne,
  outOfTen,
  readableBitrate,
  readableDate,
  readableSize,
  whatIsLeft,
  whatTheFileHolds,
} from "../readable";
import type { Wording } from "../readable";
import { elsewhere, groupCrew, useWorkScreen } from "../screens/work";
import type { Tracks } from "../screens/work";
import { useSettings } from "../settings";
import { useShownPicture } from "../components/picture";
import { asClock } from "../player/clock";
import { trackName } from "../player/describe";
import { Player } from "../player/player";
import { cutOut } from "../player/thumbnail";
import { isCatalogued, isNamed } from "../works";
import { FolderView, PhotoView } from "./own";

/** As many of the cast as the server prepares faces for: past this the names
 *  would show with an initial where the others have a picture. */
const FACES_SHOWN = 18;

export function WorkPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { t } = useSettings();
  const screen = useWorkScreen(id);
  const {
    work,
    failed,
    chosen,
    readAgain,
    playing,
    onlyToPlay,
    stopPlaying,
    trailer,
    stopTrailer,
    nextEpisode,
    previousEpisode,
    playEpisode,
  } = screen;

  /* Escape goes back, which is what a remote control and a keyboard both
     expect after opening something. Not while something is being watched: the
     player answers to escape itself, shutting a menu or leaving fullscreen
     before it shuts the film, and a page listening underneath it took a viewer
     off the film every time they shut a menu. */
  const watching = playing !== null || trailer !== null;

  /* Asked for above every way out of this function, and with nothing when
     there is nothing yet. A hook reached only once the film has arrived is a
     hook this page renders a different number of times, and React stops the
     whole page over it: this one had every fiche of the library answering
     with a blank screen. */
  const { picture: backdrop, itDidNotLoad: backdropFailed } = useShownPicture(work?.backdrop ?? []);

  useEffect(() => {
    if (watching) {
      return;
    }
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        navigate(-1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, watching]);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t(failed === "not_found" ? "error.not_found" : "error.unreachable")}</p>
      </main>
    );
  }
  /* Before anything else about the work, since the player stays through the
     moment between two episodes when the next one's description is still on
     its way: taken down and put back, it would give back a filled screen
     and every choice made in it. */
  if (playing) {
    return (
      <Player
        sourceId={playing.source}
        work={work}
        fromTheStart={playing.fromTheStart}
        startAt={playing.at}
        onClose={stopPlaying}
        /* One episode after another without anybody pressing anything, and
           stepping between them by hand, go through the same step: each
           episode is opened on its own page, which is what writes its own
           progress down and shows its own title. */
        onEnded={nextEpisode ?? undefined}
        onNextEpisode={nextEpisode ?? undefined}
        onPreviousEpisode={previousEpisode ?? undefined}
        onSelectEpisode={playEpisode}
      />
    );
  }
  /* Opened only to play, and the film is not up yet or has just been left:
     nothing of the page is drawn. Somebody who pressed play on a card never asked to see this page,
     and it used to flash past them on the way to the film. */
  if (!work || onlyToPlay) {
    return <main className="page" aria-busy="true" />;
  }
  /* What somebody filmed or photographed themselves has pages of its own: a
     folder is what it holds, and a photo is looked at rather than played. */
  if (work.kind === "folder") {
    return <FolderView work={work} />;
  }
  if (work.kind === "photo") {
    return <PhotoView work={work} />;
  }

  const version = work.versions[chosen];
  /* A series is nothing but its seasons and a season nothing but its
     episodes: neither is played, neither has a copy on the disk. */
  const holdsOthers = work.children.length > 0 || work.kind === "series" || work.kind === "season";

  return (
    <main className="work" style={{ ["--work-color" as string]: work.color ?? "var(--surface)" }}>
      {backdrop && (
        <div className="work-backdrop" aria-hidden="true">
          <img
            src={backdrop.src}
            srcSet={backdrop.srcSet}
            sizes="100vw"
            alt=""
            fetchPriority="high"
            onError={backdropFailed}
          />
        </div>
      )}

      <TopOfTheWork screen={screen} holdsOthers={holdsOthers} />

      {holdsOthers && <WhatHangsUnder work={work} />}

      {work.kind === "episode" && <TheRestOfTheSeason work={work} />}

      {work.cast.length > 0 && (
        <section className="section">
          <RowHead mark={<PeopleIcon size={24} />} title={t("work.cast")} />
          <Row>
            {work.cast.slice(0, FACES_SHOWN).map((credit, index) => (
              <PersonCard key={`${credit.person_id}-${index}`} credit={credit} />
            ))}
          </Row>
        </section>
      )}

      {!holdsOthers && version && screen.plan && (
        <Chapters
          plan={screen.plan}
          onPlay={(at) => screen.play(version.id, false, at)}
        />
      )}

      {work.alike && work.alike.cards.length > 0 && (
        <section className="section">
          <RowHead
            mark={<TagIcon size={24} />}
            title={t("work.alike", { genre: work.alike.genre })}
            to={`/library/${work.library_id}?genre=${encodeURIComponent(work.alike.genre)}`}
          />
          <Row>
            {work.alike.cards.map((card) => (
              <Card key={card.id} card={card} />
            ))}
          </Row>
        </section>
      )}

      {version && (
        <Versions
          version={version}
          /* A film held once has nothing to take this copy away from. */
          separable={work.versions.length > 1}
          onChanged={readAgain}
        />
      )}

      {/* Over the page rather than in place of it: shut, the page is where
          it was left. */}
      {trailer && <TrailerDialog trailer={trailer} title={work.title} onClose={stopTrailer} />}
    </main>
  );
}

/**
 * The top of the page, which fits the screen.
 *
 * How much room there is is read off a ruler as tall as the window under the
 * bar, and everything but the synopsis is measured as it stands: the rest is
 * the synopsis's to take, and the steps of `nextFit` decide what it does with
 * too little.
 */
function TopOfTheWork({
  screen,
  holdsOthers,
}: {
  screen: ReturnType<typeof useWorkScreen>;
  holdsOthers: boolean;
}) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const work = screen.work as Work;
  const version = work.versions[screen.chosen];
  /* An episode is shown by a still, lying down as every row shows it: stood
     up, a still is a poster cut out of the middle of a wide picture. Its own
     card among the episodes of its season carries the wide picture a row
     would draw, the still itself or failing that one of its series. A video
     of one's own lies down too: its picture is a frame of it, and stood up
     it lost both its sides. */
  const lying = work.kind === "episode" || work.kind === "video";
  const itself = work.siblings.find((sibling) => sibling.card.id === work.id)?.card;
  const { picture: poster, itDidNotLoad: posterFailed } = useShownPicture(
    itself && itself.wide.length > 0 ? itself.wide : work.poster,
  );
  const { here, away } = screen.onOffer;

  /* A season is announced by its number in the language being read, and by
     the name it was given only when that name says something the number does
     not. The server is the one that knows which is which. */
  const heading =
    nameOfOne(work.kind, work.number, work.has_own_name ? work.title : null, t) || work.title;

  /* Measured again whenever what stands above the synopsis changes: the
     tracks arriving, the bar of how far in. */
  const fitted = useFittedText([
    work.id,
    work.overview,
    screen.plan !== null,
    screen.resumeFrom !== null,
  ]);

  const facts = [
    work.year !== null ? String(work.year) : null,
    work.kind === "series" && work.children.length > 0
      ? howMany(work.children.length, "work.season_count", t)
      : null,
    work.kind === "season" && work.children.length > 0
      ? howMany(work.children.length, "work.episode_count", t)
      : null,
    !holdsOthers && work.runtime_minutes ? howLong(work.runtime_minutes, t) : null,
  ].filter((fact): fact is string => Boolean(fact));

  return (
    <section className="work-top">
      <div className={`work-poster${lying ? " work-poster-lying" : ""}`}>
        {poster ? (
          <img
            src={poster.src}
            srcSet={poster.srcSet}
            sizes={lying ? "(max-width: 800px) 60vw, 480px" : "(max-width: 800px) 40vw, 300px"}
            alt=""
            onError={posterFailed}
          />
        ) : (
          <div className="work-poster-empty" aria-hidden="true">
            {/* A season has a number and that is what it is looked for by.
                Everything else falls back to the letter it begins with. */}
            {work.number ?? heading.slice(0, 1)}
          </div>
        )}
      </div>

      <div className={`work-body${fitted.fit.wide ? " work-body-wide" : ""}`} ref={fitted.column}>
        <div className="work-room" ref={fitted.ruler} aria-hidden="true" />
        <WayBackUp work={work} />
        <h1 className="work-title">{heading}</h1>
        {work.tagline && <p className="work-tagline">{work.tagline}</p>}

        <p className="work-facts">
          {work.rating !== null && (
            <span className="work-rating">
              <StarIcon size={17} />
              {outOfTen(work.rating)}
            </span>
          )}
          {facts.length > 0 && <span>{facts.join(" · ")}</span>}
          {work.age_rating && <span className="work-badge">{work.age_rating}</span>}
          {work.genres.length > 0 && <span>{work.genres.join(", ")}</span>}
          {!isNamed(work.identification) && (
            <span className="work-badge work-badge-warning">{t("work.unidentified")}</span>
          )}
        </p>

        {/* Said in full here, where there is room for a sentence somebody can
            act on without opening a terminal. */}
        {work.identification_note && (
          <p className="notice">{t(`note.${work.identification_note}`)}</p>
        )}

        {!holdsOthers && version && (
          <Choices
            versions={work.versions}
            chosen={screen.chosen}
            choose={screen.choose}
            plan={screen.plan}
            tracks={screen.tracks}
            chooseTracks={screen.chooseTracks}
          />
        )}

        <div className="work-actions">
          {holdsOthers ? (
            <CarryOn work={work} />
          ) : (
            <>
              <button
                className="button button-accent button-large"
                disabled={!version || version.missing}
                onClick={() => version && screen.play(version.id, false)}
              >
                <span className="play-mark" aria-hidden="true" />
                {screen.resumeFrom === null ? t("work.play") : t("player.resume")}
              </button>
              {/* A separate button rather than a choice inside the player: a
                  viewer who wants to start again should not have to start
                  where they left off first. */}
              {screen.resumeFrom !== null && version && (
                <button className="button button-large" onClick={() => screen.play(version.id, true)}>
                  {t("player.from_the_start")}
                </button>
              )}
            </>
          )}
          {/* One sitting next to the film plays here; failing that, a link
              is opened where it lives. This server never goes and fetches
              someone else's video to pass it on. */}
          {here && (
            <button className="button button-large" onClick={() => screen.watchTrailer(here)}>
              <TrailerIcon size={22} />
              {t("work.trailer")}
            </button>
          )}
          {away && (
            <a className="button button-large" href={away} target="_blank" rel="noreferrer noopener">
              <TrailerIcon size={22} />
              {t("work.trailer")}
            </a>
          )}
          {/* The one after this, for whoever does not want to wait for the
              end of this one to get there. */}
          {work.kind === "episode" && work.carry_on_with && (
            <Link className="button button-large" to={`/work/${work.carry_on_with.id}`}>
              {t("work.next_episode")}
            </Link>
          )}
          {work.card && (
            <Marks
              card={work.card}
              onIdentified={screen.readAgain}
              onChanged={screen.readAgain}
              onDeleted={() => navigate(-1)}
            />
          )}
        </div>

        {!holdsOthers && (
          <HowFarIn
            seconds={screen.resumeFrom}
            minutes={screen.plan?.duration_minutes ?? work.runtime_minutes}
          />
        )}

        {/* A video of one's own has no synopsis to be missing. */}
        {isCatalogued(work.identification) &&
          (work.overview ? (
            <div className="work-synopsis">
              <p
                ref={fitted.text}
                className="work-overview"
                style={fitted.style}
                data-cut={fitted.cut ? "" : undefined}
              >
                {work.overview}
              </p>
              {fitted.fit.lines !== null && (
                <button className="link-button" onClick={fitted.toggle}>
                  {t(fitted.open ? "work.less" : "work.more")}
                </button>
              )}
            </div>
          ) : (
            <p className="work-overview work-overview-empty">{t("work.no_overview")}</p>
          ))}

        <Credits work={work} />
      </div>
    </section>
  );
}

/**
 * Which copy, which soundtrack and which subtitles it will start with.
 *
 * The copy is chosen here only when there is more than one; the tracks are
 * those of the plan the server made for the copy, and a choice is remembered
 * exactly as one made in the player is, so the player opens with it.
 */
function Choices({
  versions,
  chosen,
  choose,
  plan,
  tracks,
  chooseTracks,
}: {
  versions: Version[];
  chosen: number;
  choose: (which: number) => void;
  plan: PlaybackPlan | null;
  tracks: Tracks;
  chooseTracks: (tracks: Tracks) => void;
}) {
  const { t, language } = useSettings();
  const version = versions[chosen];
  const nameOf = (track: Parameters<typeof trackName>[0]) => trackName(track, t, language);

  const audio = plan?.audio ?? [];
  const subtitles = plan?.subtitles ?? [];

  return (
    <div className="work-choices">
      <span className="work-choice">
        <span className="work-choice-label">{t("work.video")}</span>
        {versions.length > 1 ? (
          <Picker
            label={t("work.video")}
            value={String(chosen)}
            options={versions.map(
              (entry, index) =>
                [String(index), `${pictureOf(entry)} · ${readableSize(entry.size_bytes)}`] as const,
            )}
            onPick={(value) => choose(Number(value))}
          />
        ) : (
          <span className="work-choice-value">{pictureOf(version)}</span>
        )}
      </span>

      {audio.length > 0 && (
        <span className="work-choice">
          <span className="work-choice-label">{t("work.audio")}</span>
          {audio.length > 1 ? (
            <Picker
              label={t("work.audio")}
              value={tracks.audio ?? ""}
              options={audio.map((track) => [track.id, nameOf(track)] as const)}
              onPick={(value) => chooseTracks({ ...tracks, audio: value })}
            />
          ) : (
            <span className="work-choice-value">{nameOf(audio[0])}</span>
          )}
        </span>
      )}

      {subtitles.length > 0 && (
        <span className="work-choice">
          <span className="work-choice-label">{t("work.subtitles")}</span>
          <Picker
            label={t("work.subtitles")}
            value={tracks.subtitle ?? ""}
            options={[
              ["", t("work.no_subtitles")] as const,
              ...subtitles.map((track) => [track.id, nameOf(track)] as const),
            ]}
            onPick={(value) => chooseTracks({ ...tracks, subtitle: value === "" ? null : value })}
          />
        </span>
      )}
    </div>
  );
}

/** The picture of a copy as a box says it: how sharp, what range, what
 *  codec. Nothing but the codec for a file nobody has analysed. */
function pictureOf(version: Version): string {
  const video = version.video[0];
  if (!video) {
    return containerName(version.container ?? "") || "";
  }
  return [
    ...whatTheFileHolds({ height: video.height, hdr: video.hdr, sound: null }),
    video.codec.toUpperCase(),
  ].join(" ");
}

/**
 * Watched, liked, and the menu every card of this work has.
 *
 * Marked through the card of the work, which is what makes the tick pressed
 * here show on every row that holds the same work, and theirs show here.
 */
function Marks({
  card,
  onIdentified,
  onChanged,
  onDeleted,
}: {
  card: CardData;
  onIdentified: () => void;
  onChanged: () => void;
  onDeleted: () => void;
}) {
  const { t } = useSettings();
  const marks = useMarks();
  const seen = marks.seenOf(card) === "watched";
  const favourite = marks.favouriteOf(card);
  const kebab = useRef<HTMLButtonElement>(null);
  const menu = useWorkMenu(card, {
    identified: onIdentified,
    picturesChanged: onChanged,
    deleted: onDeleted,
  });

  const seenSaid = t(seen ? "card.menu.mark_unwatched" : "card.menu.mark_watched");
  const favouriteSaid = t(favourite ? "card.unfavourite" : "card.favourite");

  return (
    <span className="work-marks">
      <button
        type="button"
        className={`work-mark${seen ? " work-mark-on" : ""}`}
        aria-pressed={seen}
        aria-label={seenSaid}
        title={seenSaid}
        onClick={() => marks.setWatched(card, !seen)}
      >
        <TickIcon size={22} />
      </button>
      <button
        type="button"
        className={`work-mark${favourite ? " work-mark-on" : ""}`}
        aria-pressed={favourite}
        aria-label={favouriteSaid}
        title={favouriteSaid}
        onClick={() => marks.setFavourite(card, !favourite)}
      >
        <HeartIcon size={22} filled={favourite} />
      </button>
      <button
        ref={kebab}
        type="button"
        className={`work-mark${menu.open ? " work-mark-on" : ""}`}
        aria-label={t("card.more")}
        title={t("card.more")}
        aria-expanded={menu.open}
        onClick={() => menu.toggle(kebab.current)}
      >
        <MoreIcon size={22} />
      </button>
      {menu.drawn}
    </span>
  );
}

/** How far in it already is, under the buttons that carry on with it: the
 *  bar, and what is left of it. */
function HowFarIn({ seconds, minutes }: { seconds: number | null; minutes: number | null }) {
  const { t } = useSettings();
  if (seconds === null || !minutes) {
    return null;
  }
  const whole = minutes * 60;
  return (
    <p className="progress-line work-progress">
      <span className="progress-bar" aria-hidden="true">
        <span
          className="progress-done"
          style={{ width: `${(Math.min(seconds, whole) / whole) * 100}%` }}
        />
      </span>
      <span>{whatIsLeft(seconds, minutes, t)}</span>
    </p>
  );
}

/**
 * Who made it, what made it, and where else it is described.
 *
 * What other servers call "about": the things that belong to the work rather
 * than to the copy of it on disk.
 */
function Credits({ work }: { work: Work }) {
  const { t } = useSettings();
  const links = work.external_ids.filter((entry) => elsewhere(entry.provider, entry.id, work.kind));

  if (work.crew.length === 0 && work.studios.length === 0 && links.length === 0 && !work.collection) {
    return null;
  }
  return (
    <dl className="work-credits">
      {groupCrew(work.crew).map(([role, names]) => (
        <div key={role} className="work-credit">
          <dt>{t(`credit.${role}`)}</dt>
          <dd>{names.join(", ")}</dd>
        </div>
      ))}
      {work.studios.length > 0 && (
        <div className="work-credit">
          <dt>{t("media.studios")}</dt>
          <dd>{work.studios.join(", ")}</dd>
        </div>
      )}
      {work.collection && (
        <div className="work-credit">
          <dt>{t("work.saga")}</dt>
          <dd>{work.collection}</dd>
        </div>
      )}
      {/* Only the ones that really lead somewhere: a season and an episode
          carry an identifier at the catalogue too, and the page that shows
          one cannot be built from it alone. */}
      {links.length > 0 && (
        <div className="work-credit">
          <dt>{t("media.links")}</dt>
          <dd className="work-links">
            {links.map((entry) => (
              <a
                key={entry.provider}
                className="work-link"
                href={elsewhere(entry.provider, entry.id, work.kind)}
                target="_blank"
                rel="noreferrer noopener"
              >
                {t(`provider.${entry.provider}`)}
              </a>
            ))}
          </dd>
        </div>
      )}
    </dl>
  );
}

/**
 * The places the film changes scene, as a row of stills.
 *
 * Only the chapters the file names itself, each shown with the little picture
 * of its first moment, cut out of the sheets the bar of the player already
 * uses. A press starts the film there.
 */
function Chapters({ plan, onPlay }: { plan: PlaybackPlan; onPlay: (at: number) => void }) {
  const { t } = useSettings();
  if (plan.chapters.length === 0) {
    return null;
  }
  return (
    <section className="section">
      <RowHead mark={<FilmIcon size={24} />} title={t("work.chapters_row")} />
      <Row>
        {plan.chapters.map((chapter, index) => {
          const name = chapter.title ?? t("work.chapter", { number: index + 1 });
          const still = plan.thumbnails ? cutOut(plan.thumbnails, chapter.at_second) : null;
          return (
            <article key={chapter.at_second} className="card card-lying" data-card>
              <div className="card-picture">
                {still && plan.thumbnails ? (
                  /* The film's own shape inside the card's, with bands
                     where they differ, as a screen shows a film. Stretched
                     to the card, a wide film came out squeezed thin. */
                  <span className="chapter-frame" aria-hidden="true">
                    <span
                      className="chapter-still"
                      style={{
                        ...still,
                        ["--film-shape" as string]:
                          plan.thumbnails.width / plan.thumbnails.height,
                      }}
                    />
                  </span>
                ) : (
                  <span className="card-initial" aria-hidden="true">
                    {index + 1}
                  </span>
                )}
                <button
                  type="button"
                  className="card-open"
                  title={t("work.play_chapter", { name })}
                  aria-label={t("work.play_chapter", { name })}
                  onClick={() => onPlay(chapter.at_second)}
                />
              </div>
              <span className="card-line">
                <span className="card-title">{name}</span>
              </span>
              <span className="card-year">{asClock(chapter.at_second)}</span>
            </article>
          );
        })}
      </Row>
    </section>
  );
}

/**
 * What the file holds, at the very end, drawn as the panels of the
 * administration are: one for the file, one per track.
 *
 * Everything the analysis recorded and nothing hidden behind a summary, in the
 * order somebody reads when something is wrong with a file: where it is
 * first, then what is inside.
 */
function Versions({
  version,
  separable,
  onChanged,
}: {
  version: Version;
  separable: boolean;
  onChanged: () => void;
}) {
  const { t, language } = useSettings();
  /* Reading a copy again and detaching it are the administrator's, which the
     server says too: a button that is refused when pressed is not offered. */
  const { account } = useAccount();

  return (
    <section className="section">
      <RowHead mark={<FolderIcon size={24} />} title={t("work.versions")} />
      <div className="panels work-panels">
        <Panel
          icon={FolderIcon}
          title={t("work.file")}
          lead={[pictureOf(version), readableSize(version.size_bytes)].filter(Boolean).join(" · ")}
          action={
            account?.is_administrator && (
              <>
                <ReadCopyAgain copy={version.id} onRead={onChanged} />
                {separable && <DetachCopy copy={version.id} onDetached={onChanged} />}
              </>
            )
          }
        >
          {(version.missing || !version.analysed) && (
            <p className="work-badges">
              {version.missing && (
                <span className="work-badge work-badge-warning">{t("work.missing")}</span>
              )}
              {!version.analysed && <span className="work-badge">{t("work.not_analysed")}</span>}
            </p>
          )}
          <dl className="work-facts-list">
            <Fact label={t("media.path")} value={version.path} wide />
            <Fact label={t("media.disk")} value={version.root_label} />
            <Fact label={t("media.added")} value={readableDate(version.added_at, language)} />
            <Fact
              label={t("media.container")}
              value={version.container ? containerName(version.container) : null}
            />
            <Fact label={t("media.bitrate")} value={readableBitrate(version.overall_bitrate)} />
            <Fact
              label={t("work.chapters_row")}
              value={version.chapters > 0 ? version.chapters : null}
            />
          </dl>
        </Panel>

        {version.video.map((track, index) => (
          <Panel
            key={`v${index}`}
            icon={FilmIcon}
            title={t("work.video")}
            lead={[pictureOf({ ...version, video: [track] }), track.title].filter(Boolean).join(" · ")}
          >
            <dl className="work-facts-list">
              <Fact label={t("track.codec")} value={track.codec.toUpperCase()} />
              <Fact label={t("track.profile")} value={track.profile} />
              <Fact label={t("track.level")} value={track.level} />
              <Fact label={t("track.resolution")} value={`${track.width}×${track.height}`} />
              <Fact label={t("track.aspect")} value={track.aspect_ratio} />
              <Fact label={t("track.interlaced")} value={t(track.is_interlaced ? "yes" : "no")} />
              <Fact
                label={t("track.frame_rate")}
                value={track.frame_rate === null ? null : track.frame_rate.toFixed(3)}
              />
              <Fact label={t("track.bitrate")} value={readableBitrate(track.bitrate)} />
              <Fact label={t("track.hdr")} value={track.hdr?.toUpperCase()} />
              <Fact label={t("track.primaries")} value={track.color_primaries} />
              <Fact label={t("track.space")} value={track.color_space} />
              <Fact label={t("track.transfer")} value={track.color_transfer} />
              <Fact
                label={t("track.depth")}
                value={track.bit_depth === null ? null : t("track.bits", { count: track.bit_depth })}
              />
              <Fact label={t("track.pixels")} value={track.pixel_format} />
              <Fact label={t("track.reference_frames")} value={track.reference_frames} />
              <Fact label={t("track.default")} value={t(track.is_default ? "yes" : "no")} />
            </dl>
          </Panel>
        ))}

        {version.audio.map((track, index) => (
          <Panel
            key={`a${index}`}
            icon={SoundIcon}
            title={t("work.audio")}
            lead={[track.language && languageName(track.language, language), track.title].filter(Boolean).join(" · ") || undefined}
          >
            <dl className="work-facts-list">
              <Fact label={t("track.codec")} value={track.codec.toUpperCase()} />
              <Fact label={t("track.profile")} value={track.profile} />
              <Fact label={t("track.layout")} value={track.channel_layout} />
              <Fact label={t("track.channels")} value={t("track.ch", { count: track.channels })} />
              <Fact label={t("track.bitrate")} value={readableBitrate(track.bitrate)} />
              <Fact
                label={t("track.sample_rate")}
                value={
                  track.sample_rate === null ? null : `${track.sample_rate.toLocaleString(language)} Hz`
                }
              />
              <Fact
                label={t("track.depth")}
                value={track.bit_depth === null ? null : t("track.bits", { count: track.bit_depth })}
              />
              <Fact label={t("track.default")} value={t(track.is_default ? "yes" : "no")} />
              <Fact label={t("track.forced")} value={t(track.is_forced ? "yes" : "no")} />
            </dl>
          </Panel>
        ))}

        {version.subtitles.map((track, index) => (
          <Panel
            key={`s${index}`}
            icon={SubtitlesIcon}
            title={t("work.subtitles")}
            lead={[track.language && languageName(track.language, language), track.title].filter(Boolean).join(" · ") || undefined}
          >
            <dl className="work-facts-list">
              <Fact label={t("track.codec")} value={track.codec.toUpperCase()} />
              <Fact label={t("track.default")} value={t(track.is_default ? "yes" : "no")} />
              <Fact label={t("track.forced")} value={t(track.is_forced ? "yes" : "no")} />
              <Fact
                label={t("track.hearing_impaired")}
                value={t(track.is_hearing_impaired ? "yes" : "no")}
              />
              <Fact label={t("track.external")} value={t(track.is_external ? "yes" : "no")} />
              {/* Not a property of the file but of what playing it would
                  cost, and the one thing here worth knowing before pressing
                  play. */}
              {track.burns_in && <Fact label={t("track.burns_in")} value={t("yes")} />}
            </dl>
          </Panel>
        ))}
      </div>
    </section>
  );
}

/** One line of a panel, left out entirely when the analysis found nothing. */
function Fact({
  label,
  value,
  wide,
}: {
  label: string;
  value: string | number | null | undefined;
  wide?: boolean;
}) {
  if (value === null || value === undefined || value === "") {
    return null;
  }
  return (
    <div className={wide ? "fact-line fact-line-wide" : "fact-line"}>
      <dt>{label}</dt>
      <dd>{value}</dd>
    </div>
  );
}

/**
 * Saying that a copy is not the same film as the one it sits on.
 *
 * Copies are put together without anybody asking, by the rules that read names
 * and by what the provider answers, and both can be wrong about one file.
 * Whoever is looking at the page can see it at a glance, so the way to say so
 * belongs on that page.
 */
function DetachCopy({ copy, onDetached }: { copy: string; onDetached: () => void }) {
  const { t } = useSettings();
  const [detached, setDetached] = useState(false);
  const told = useTold(async () => {
    await api.detachCopy(copy);
    /* Kept quiet from here on: the film is about to be read again without
       this copy, and a button that comes back to life in between is a button
       somebody presses twice. */
    setDetached(true);
    onDetached();
  });
  const busy = told.busy || detached;

  return (
    <>
      <button className="button button-small" onClick={() => told.tell()} disabled={busy}>
        {busy ? t("detach.busy") : t("detach.open")}
      </button>
      {told.failure && <span className="notice">{t("detach.failed")}</span>}
    </>
  );
}

/**
 * Asking for one file to be read again for what it says about itself.
 *
 * A scan opens only a file whose size or date changed on disk, so a server
 * that has learnt to read something new out of a file can never reach the ones
 * it has already described. Without this the only way to profit from such an
 * improvement on a library already scanned is to touch the files by hand or to
 * describe the whole collection again, which takes hours.
 *
 * What comes back is a job like any other, so the reading shows itself on the
 * activity screen and says when it is done.
 */
function ReadCopyAgain({ copy, onRead }: { copy: string; onRead: () => void }) {
  const { t } = useSettings();
  const told = useTold(async () => {
    await api.readCopyAgain(copy);
    onRead();
  });

  return (
    <>
      <button className="button button-small" onClick={() => told.tell()} disabled={told.busy}>
        {told.busy ? t("read_again.busy") : t("read_again.open")}
      </button>
      {told.failure && <span className="notice">{t("read_again.failed")}</span>}
    </>
  );
}

/**
 * What hangs under this one: the seasons of a series, the episodes of a season.
 *
 * Seasons are a row of cards, because a season is chosen by its poster the
 * way a film is. Episodes are a list, each still beside its name, its length
 * and what it is about, because an episode is chosen by what happens in it.
 * Both are drawn with the card every row draws, so they grow, light up, play
 * and open their menu exactly as every other card does.
 */
function WhatHangsUnder({ work }: { work: Work }) {
  const { t } = useSettings();
  const seasons = work.kind === "series";

  if (work.children.length === 0) {
    return (
      <p className="notice">{t(seasons ? "work.seasons_none" : "work.episodes_none")}</p>
    );
  }

  return (
    <section className="section">
      <RowHead
        mark={seasons ? <CollectionIcon size={24} /> : <PlaylistIcon size={24} />}
        title={t(seasons ? "work.seasons" : "work.episodes")}
      />
      {seasons ? (
        <Row>
          {work.children.map((child) => (
            <Card
              key={child.card.id}
              card={child.card}
              lead={nameOfChild(child, t)}
              note={howMany(child.child_count, "work.episode_count", t)}
            />
          ))}
        </Row>
      ) : (
        <ol className="episode-lines">
          {work.children.map((child) => (
            <EpisodeLine key={child.card.id} child={child} />
          ))}
        </ol>
      )}
    </section>
  );
}

/** One episode of a season's list: its still, and beside it what it is
 *  called, how it is rated, how long it runs and what it is about. */
function EpisodeLine({ child }: { child: Child }) {
  const { t } = useSettings();
  const { card } = child;
  const left =
    card.resume_from_seconds !== null
      ? whatIsLeft(card.resume_from_seconds, card.runtime_minutes, t)
      : undefined;

  return (
    <li className="episode-line">
      <Card card={card} shape="lying" named={false} />
      <div className="episode-line-words">
        <Link className="episode-line-name" to={`/work/${card.id}`}>
          {nameOfChild(child, t)}
        </Link>
        <p className="work-facts">
          {card.rating !== null && (
            <span className="work-rating">
              <StarIcon size={15} />
              {outOfTen(card.rating)}
            </span>
          )}
          <span>{lengthOf(card, t)}</span>
          {left && <span>{left}</span>}
        </p>
        {child.overview && <p className="episode-line-overview">{child.overview}</p>}
      </div>
    </li>
  );
}

/**
 * Every episode of the season this one belongs to, this one lit among them:
 * the way to the one before and the ones after without going back up to the
 * season. The row opens where this one is, and its heading leads to the
 * season itself.
 */
function TheRestOfTheSeason({ work }: { work: Work }) {
  const { t } = useSettings();
  const season = work.ancestry.find((up) => up.kind === "season");
  /* A season of one episode is this page again, as a row of one card. */
  if (!season || work.siblings.length < 2) {
    return null;
  }
  const here = work.siblings.findIndex((sibling) => sibling.card.id === work.id);

  return (
    <section className="section">
      <RowHead
        mark={<PlaylistIcon size={24} />}
        title={nameOfOne(season.kind, season.number, season.title, t) || t("work.episodes")}
        to={`/work/${season.id}`}
      />
      {/* Drawn anew for another season, so it opens on its own episode
          rather than keeping where the last season's row was left. */}
      <Row key={season.id} opensOn={here < 0 ? undefined : here}>
        {work.siblings.map((sibling) => (
          <Card
            key={sibling.card.id}
            card={sibling.card}
            shape="lying"
            here={sibling.card.id === work.id}
            lead={nameOfChild(sibling, t)}
            note={lengthOf(sibling.card, t)}
          />
        ))}
      </Row>
    </section>
  );
}

/** What one season or episode is called: its number in the language being
 *  read, and its own name when it has one the number does not say. */
function nameOfChild(child: Child, t: Wording): string {
  return nameOfOne(child.card.kind, child.number, child.title, t) || child.card.title;
}

/** How long an episode runs, or that there is no file of it to play. */
function lengthOf(card: CardData, t: Wording): string {
  if (card.source === null) {
    return t("work.not_on_disk");
  }
  return card.runtime_minutes ? howLong(card.runtime_minutes, t) : "";
}

/**
 * What a series or a season offers to play next.
 *
 * One press, and the episode's own page opens and starts it. Not played from
 * here: the episode is what is being watched, so its page is what shows its
 * title and writes its own progress down.
 */
function CarryOn({ work }: { work: Work }) {
  const { t } = useSettings();
  const next = work.carry_on_with;

  if (!next) {
    /* Nothing left only says something once there is something here at all. */
    return work.children.length > 0 ? (
      <p className="work-all-watched">{t("work.all_watched")}</p>
    ) : null;
  }

  /* Nothing to play it from, or no way to say which episode it is, means
     nothing to offer: the list of episodes is right below either way, and a
     button that cannot name what it would start is worse than no button. */
  if (!next.source_id || next.season === null || next.episode === null) {
    return null;
  }

  /* Whether anybody has started this at all, which decides between starting
     and carrying on. A season page counts watched episodes and a series page
     counts what is left inside each season: an episode has nothing under it,
     so counting its children would say every season page is untouched. */
  const untouched = work.children.every(({ card }) =>
    card.kind === "episode" ? card.seen !== "watched" : card.unwatched === card.episodes,
  );
  const wording = t(untouched ? "work.start_series" : "work.carry_on", {
    season: String(next.season).padStart(2, "0"),
    episode: String(next.episode).padStart(2, "0"),
  });

  return (
    <>
      <Link className="button button-accent button-large" to={`/work/${next.id}?play=1`}>
        <span className="play-mark" aria-hidden="true" />
        {wording}
      </Link>
      {next.title && <span className="carry-on-title">{next.title}</span>}
    </>
  );
}
