/*
 * One film.
 *
 * The synopsis is shown whole when there is room for it. The button only
 * appears when the text really is cut, which is measured rather than guessed:
 * a page that always clamps leaves an empty half and a button that does
 * nothing, which is the thing the maintainer disliked about the others.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import type { Child, Credit, Version, Work } from "../api";
import { useTold } from "../asking";
import { IdentifyByHand } from "../components/byhand";
import {
  howMany,
  nameOfOne,
  numberOfOne,
  outOfTen,
  readableBitrate,
  readableDate,
  readableSize,
} from "../readable";
import { elsewhere, groupCrew, useWorkScreen } from "../screens/work";
import { useSettings } from "../settings";
import { useShownPicture } from "../components/picture";
import { Player } from "../player/player";
import { TrailerPlayer } from "../player/trailer";

export function WorkPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { t } = useSettings();
  const {
    work,
    failed,
    chosen,
    choose,
    resumeFrom,
    readAgain,
    playing,
    openingToPlay,
    play,
    stopPlaying,
    onOffer,
    trailer,
    watchTrailer,
    stopTrailer,
    nextEpisode,
    previousEpisode,
    playEpisode,
  } = useWorkScreen(id);

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
  const { picture: backdrop, itDidNotLoad: backdropFailed } = useShownPicture(
    work?.backdrop ?? [],
  );
  const { picture: poster, itDidNotLoad: posterFailed } = useShownPicture(work?.poster ?? []);

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
  /* Opened only to play, and the film is not up yet: nothing of the page is
     drawn. Somebody who pressed play on a card never asked to see this page,
     and it used to flash past them on the way to the film. */
  if (!work || openingToPlay) {
    return <main className="page" aria-busy="true" />;
  }

  const { here, away } = onOffer;
  const version = work.versions[chosen];
  /* A season is announced by its number in the language being read, and by
     the name it was given only when that name says something the number does
     not. The server is the one that knows which is which. */
  const heading =
    nameOfOne(work.kind, work.number, work.has_own_name ? work.title : null, t) || work.title;
  /* A series is nothing but its seasons and a season nothing but its
     episodes: neither is played, neither has a copy on the disk, and neither
     runs for a length of its own. */
  const holdsOthers = work.children.length > 0 || work.kind === "series" || work.kind === "season";
  const facts = [
    work.year !== null ? String(work.year) : null,
    work.kind === "series" && work.children.length > 0
      ? howMany(work.children.length, "work.season_count", t)
      : null,
    work.kind === "season" && work.children.length > 0
      ? howMany(work.children.length, "work.episode_count", t)
      : null,
    !holdsOthers && work.runtime_minutes
      ? t("work.minutes", { count: work.runtime_minutes })
      : null,
    work.age_rating,
    work.rating !== null ? outOfTen(work.rating) : null,
  ].filter((fact): fact is string => Boolean(fact));

  if (playing) {
    return (
      <Player
        sourceId={playing.source}
        work={work}
        fromTheStart={playing.fromTheStart}
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

  if (trailer) {
    return (
      <TrailerPlayer url={trailer} title={work.title} onClose={() => stopTrailer()} />
    );
  }

  return (
    <main className="work" style={{ ["--work-color" as string]: work.color ?? "var(--surface)" }}>
      {backdrop && (
        <div className="work-backdrop" aria-hidden="true">
          <img
            src={backdrop.src}
            srcSet={backdrop.srcSet}
            sizes="100vw"
            alt=""
            onError={backdropFailed}
          />
          <div className="work-scrim" />
        </div>
      )}

      <div className="work-inner">
        <div className="work-poster">
          {poster ? (
            <img
              src={poster.src}
              srcSet={poster.srcSet}
              sizes="(max-width: 800px) 40vw, 300px"
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

        <div className="work-body">
          {/* The way back up, drawn before anything else, out of what came
              with the page rather than out of a second question. */}
          {work.ancestry.length > 0 && (
            <nav className="work-ancestry">
              {[...work.ancestry].reverse().map((up) => (
                <Link key={up.id} to={`/work/${up.id}`} className="work-ancestor">
                  {nameOfOne(up.kind, up.number, up.title, t) || up.title}
                </Link>
              ))}
            </nav>
          )}
          <h1 className="work-title">{heading}</h1>
          {work.tagline && <p className="work-tagline">{work.tagline}</p>}

          <div className="work-facts">
            {facts.map((fact) => (
              <span key={fact} className="fact">
                {fact}
              </span>
            ))}
            {work.identification !== "identified" && work.identification !== "manual" && (
              <span className="fact fact-warning">{t("work.unidentified")}</span>
            )}
          </div>

          {/* Said in full here, where there is room for a sentence somebody can
              act on without opening a terminal. */}
          {work.identification_note && (
            <p className="notice">{t(`note.${work.identification_note}`)}</p>
          )}

          {/* The last word, on every film and not only the nameless ones: a
              film named wrongly looks exactly like one named rightly, and the
              person looking at it is the only one who can tell. */}
          {id && work.kind === "movie" && (
            <IdentifyByHand
              workId={id}
              title={work.title}
              onIdentified={() => readAgain()}
            />
          )}

          {!holdsOthers && (
          <div className="work-actions">
            <button
              className="button button-accent button-large"
              disabled={!version || version.missing}
              onClick={() =>
                version && play(version.id, false)
              }
            >
              <span className="play-mark" aria-hidden="true" />
              {resumeFrom === null ? t("work.play") : t("player.resume")}
            </button>
            {/* A separate button rather than a choice inside the player: a
                viewer who wants to start again should not have to start where
                they left off first. */}
            {resumeFrom !== null && (
              <button
                className="button"
                onClick={() =>
                  version && play(version.id, true)
                }
              >
                {t("player.from_the_start")}
              </button>
            )}
            {/* One sitting next to the film plays here; failing that, a link
                is opened where it lives. This server never goes and fetches
                someone else's video to pass it on. */}
            {here && (
              <button className="button" onClick={() => watchTrailer(here)}>
                {t("work.trailer")}
              </button>
            )}
            {away && (
              <a className="button" href={away} target="_blank" rel="noreferrer noopener">
                {t("work.trailer")}
              </a>
            )}
            {/* The one after this, for whoever does not want to wait for the
                end of this one to get there. */}
            {work.kind === "episode" && work.carry_on_with && (
              <Link className="button" to={`/work/${work.carry_on_with.id}`}>
                {t("work.next_episode")}
              </Link>
            )}
          </div>
          )}

          <Synopsis text={work.overview} />

          {holdsOthers && <CarryOn work={work} />}

          {holdsOthers && <WhatHangsUnder work={work} />}

          {work.genres.length > 0 && (
            <div className="pills">
              {work.genres.map((genre) => (
                <span key={genre} className="pill">
                  {genre}
                </span>
              ))}
            </div>
          )}

          {work.collection && (
            <p className="work-collection">
              {t("work.collection", { name: work.collection })}
            </p>
          )}

          {/* What Emby calls "about": the things that belong to the film rather
              than to the copy of it on disk. */}
          {work.studios.length > 0 && (
            <p className="work-studios">
              <span className="work-studios-label">{t("media.studios")}</span>
              {work.studios.join(", ")}
            </p>
          )}

          {/* Only the ones that really lead somewhere: a season and an
              episode carry an identifier at the catalogue too, and the page
              that shows one cannot be built from it alone. */}
          {work.external_ids.some((entry) => elsewhere(entry.provider, entry.id, work.kind)) && (
            <p className="work-links">
              <span className="work-studios-label">{t("media.links")}</span>
              {work.external_ids
                .filter((entry) => elsewhere(entry.provider, entry.id, work.kind))
                .map((entry) => (
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
            </p>
          )}

          {work.crew.length > 0 && (
            <dl className="work-crew">
              {groupCrew(work.crew).map(([role, names]) => (
                <div key={role} className="crew-line">
                  <dt>{t(`credit.${role}`)}</dt>
                  <dd>{names.join(", ")}</dd>
                </div>
              ))}
            </dl>
          )}
        </div>
      </div>

      {work.cast.length > 0 && (
        <section className="section">
          <h2>{t("work.cast")}</h2>
          <div className="cast-row">
            {/* As many as the server prepares faces for: past this the names
                would show with an initial where the others have a picture. */}
            {work.cast.slice(0, 18).map((credit, index) => (
              <div key={`${credit.name}-${index}`} className="cast-card">
                <Face credit={credit} />
                <span className="cast-name">{credit.name}</span>
                {credit.character && <span className="cast-role">{credit.character}</span>}
              </div>
            ))}
          </div>
        </section>
      )}

      {work.versions.length > 0 && (
        <section className="section">
          <h2>{t("work.versions")}</h2>
          {work.versions.length > 1 && (
            <div className="version-tabs" role="tablist">
              {work.versions.map((entry, index) => (
                <button
                  key={entry.id}
                  role="tab"
                  aria-selected={index === chosen}
                  className={`toggle ${index === chosen ? "toggle-on" : ""}`}
                  onClick={() => choose(index)}
                >
                  {entry.summary || `${t("work.version")} ${index + 1}`}
                </button>
              ))}
            </div>
          )}
          {version && (
            <VersionDetails
              version={version}
              /* A film held once has nothing to take this copy away from. */
              separable={work.versions.length > 1}
              onDetached={() => readAgain()}
            />
          )}
        </section>
      )}
    </main>
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
 * The face of one person, or their initial while there is none.
 *
 * The frame is the same either way, so a row of faces where one picture is
 * missing keeps its line rather than shifting everything after it.
 */
function Face({ credit }: { credit: Credit }) {
  const { picture: photo, itDidNotLoad } = useShownPicture(credit.photo);
  if (!photo) {
    return (
      <div className="cast-face" aria-hidden="true">
        {credit.name.slice(0, 1)}
      </div>
    );
  }

  return (
    <div className="cast-face">
      <img
        src={photo.src}
        srcSet={photo.srcSet}
        sizes="96px"
        alt=""
        loading="lazy"
        onError={itDidNotLoad}
      />
    </div>
  );
}

/**
 * The synopsis, whole when it fits.
 *
 * Whether it fits is measured after the text is laid out, at the width it
 * really has, rather than counted in characters, which is wrong at every
 * width but one.
 */
function Synopsis({ text }: { text: string | null }) {
  const { t } = useSettings();
  const paragraph = useRef<HTMLParagraphElement>(null);
  const [cut, setCut] = useState(false);
  const [open, setOpen] = useState(false);

  useLayoutEffect(() => {
    const element = paragraph.current;
    if (!element) {
      return;
    }
    const measure = () => {
      // A pixel of slack: a line of text is rarely a whole number of pixels.
      setCut(element.scrollHeight > element.clientHeight + 1);
    };
    measure();

    const watcher = new ResizeObserver(measure);
    watcher.observe(element);
    return () => watcher.disconnect();
  }, [text]);

  if (!text) {
    return <p className="work-overview work-overview-empty">{t("work.no_overview")}</p>;
  }

  return (
    <div className="work-synopsis">
      <p ref={paragraph} className={`work-overview ${open ? "work-overview-open" : ""}`}>
        {text}
      </p>
      {(cut || open) && (
        <button className="link-button" onClick={() => setOpen(!open)}>
          {t(open ? "work.less" : "work.more")}
        </button>
      )}
    </div>
  );
}

function VersionDetails({
  version,
  separable,
  onDetached,
}: {
  version: Version;
  separable: boolean;
  onDetached: () => void;
}) {
  const { t, language } = useSettings();

  /* Everything about one file, laid out the way somebody reads it when
     something is wrong with that file: where it is first, then one card per
     track with every field the analysis recorded. Nothing is hidden behind a
     summary: a summary is what the line above the play button is for. */
  return (
    <div className="version">
      <p className="version-line">
        <span className="version-summary">{version.summary}</span>
        <span className="version-size">{readableSize(version.size_bytes)}</span>
        {version.container && <span className="version-size">{version.container}</span>}
        {version.chapters > 0 && (
          <span className="version-size">{t("work.chapters", { count: version.chapters })}</span>
        )}
        {version.missing && <span className="fact fact-warning">{t("work.missing")}</span>}
        {!version.analysed && <span className="fact">{t("work.not_analysed")}</span>}
      </p>

      <p className="version-actions">
        <ReadCopyAgain copy={version.id} onRead={onDetached} />
        {separable && <DetachCopy copy={version.id} onDetached={onDetached} />}
      </p>

      <dl className="media-facts">
        <Fact label={t("media.path")} value={version.path} wide />
        <Fact label={t("media.disk")} value={version.root_label} />
        <Fact label={t("media.added")} value={readableDate(version.added_at, language)} />
        <Fact label={t("media.bitrate")} value={readableBitrate(version.overall_bitrate)} />
      </dl>

      <div className="track-cards">
        {version.video.map((track, index) => (
          <TrackCard key={`v${index}`} heading={t("work.video")}>
            <Fact label={t("track.title")} value={track.title} />
            <Fact label={t("track.codec")} value={track.codec.toUpperCase()} />
            <Fact label={t("track.profile")} value={track.profile} />
            <Fact label={t("track.level")} value={track.level} />
            <Fact label={t("track.resolution")} value={`${track.width}\u00d7${track.height}`} />
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
          </TrackCard>
        ))}

        {version.audio.map((track, index) => (
          <TrackCard key={`a${index}`} heading={t("work.audio")}>
            <Fact label={t("track.title")} value={track.title} />
            <Fact label={t("track.language")} value={track.language} />
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
          </TrackCard>
        ))}

        {version.subtitles.map((track, index) => (
          <TrackCard key={`s${index}`} heading={t("work.subtitles")}>
            <Fact label={t("track.title")} value={track.title} />
            <Fact label={t("track.language")} value={track.language} />
            <Fact label={t("track.codec")} value={track.codec.toUpperCase()} />
            <Fact label={t("track.default")} value={t(track.is_default ? "yes" : "no")} />
            <Fact label={t("track.forced")} value={t(track.is_forced ? "yes" : "no")} />
            <Fact
              label={t("track.hearing_impaired")}
              value={t(track.is_hearing_impaired ? "yes" : "no")}
            />
            <Fact label={t("track.external")} value={t(track.is_external ? "yes" : "no")} />
            {/* Not a property of the file but of what playing it would cost,
                and the one thing here worth knowing before pressing play. */}
            {track.burns_in && <Fact label={t("track.burns_in")} value={t("yes")} />}
          </TrackCard>
        ))}
      </div>
    </div>
  );
}

function TrackCard({ heading, children }: { heading: string; children: React.ReactNode }) {
  return (
    <div className="track-card">
      <h3>{heading}</h3>
      <dl className="track-facts">{children}</dl>
    </div>
  );
}

/** One line of a card, left out entirely when the analysis found nothing. */
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
 * What hangs under this one: the seasons of a series, the episodes of a season.
 *
 * Seasons are cards, because a season is chosen by its poster the way a film
 * is. Episodes are rows, because an episode is chosen by its number and its
 * name and there are twenty four of them.
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
    <section className="work-children">
      <h2 className="work-section">{t(seasons ? "work.seasons" : "work.episodes")}</h2>
      {seasons ? (
        <div className="season-grid">
          {work.children.map((child) => (
            <SeasonCard key={child.id} child={child} />
          ))}
        </div>
      ) : (
        <ol className="episode-list">
          {work.children.map((child) => (
            <EpisodeRow key={child.id} child={child} />
          ))}
        </ol>
      )}
    </section>
  );
}

function SeasonCard({ child }: { child: Child }) {
  const { t } = useSettings();
  const { picture: poster, itDidNotLoad } = useShownPicture(child.poster);

  return (
    <Link
      to={`/work/${child.id}`}
      className="season-card"
      style={{ ["--card-color" as string]: child.color ?? "var(--surface)" }}
    >
      <div className="season-poster">
        {poster ? (
          <img
            src={poster.src}
            srcSet={poster.srcSet}
            sizes="200px"
            alt=""
            onError={itDidNotLoad}
          />
        ) : (
          <div className="season-poster-empty" aria-hidden="true">
            {child.number ?? ""}
          </div>
        )}
      </div>
      <span className="season-name">{numberOfOne(child.kind, child.number, t)}</span>
      {child.title && <span className="season-title">{child.title}</span>}
      <span className="season-count">
        {child.unwatched > 0
          ? howMany(child.unwatched, "work.left_to_watch", t)
          : howMany(child.child_count, "work.episode_count", t)}
      </span>
    </Link>
  );
}

function EpisodeRow({ child }: { child: Child }) {
  const { t } = useSettings();

  return (
    <li className="episode">
      <Link to={`/work/${child.id}`} className="episode-link">
        <span className="episode-number">{child.number ?? ""}</span>
        <span className="episode-name">
          {child.title ?? numberOfOne(child.kind, child.number, t)}
        </span>
        <span className="episode-length">
          {!child.playable
            ? t("work.not_on_disk")
            : child.runtime_minutes
              ? t("work.minutes", { count: child.runtime_minutes })
              : ""}
        </span>
        {/* A tick rather than a word: a list of twenty four lines each ending
            in "Vu" reads as a wall of the same word. */}
        {child.watched && (
          <span className="episode-watched" title={t("work.watched")} aria-label={t("work.watched")}>
            ✓
          </span>
        )}
      </Link>
    </li>
  );
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
  const untouched = work.children.every((child) =>
    child.kind === "episode" ? !child.watched : child.unwatched === child.child_count,
  );
  const wording = t(untouched ? "work.start_series" : "work.carry_on", {
    season: String(next.season).padStart(2, "0"),
    episode: String(next.episode).padStart(2, "0"),
  });

  return (
    <div className="work-actions">
      <Link className="button button-accent button-large" to={`/work/${next.id}?play=1`}>
        <span className="play-mark" aria-hidden="true" />
        {wording}
      </Link>
      {next.title && <span className="carry-on-title">{next.title}</span>}
    </div>
  );
}
