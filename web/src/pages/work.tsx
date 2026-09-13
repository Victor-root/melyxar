/*
 * One film.
 *
 * The synopsis is shown whole when there is room for it. The button only
 * appears when the text really is cut, which is measured rather than guessed:
 * a page that always clamps leaves an empty half and a button that does
 * nothing, which is the thing the maintainer disliked about the others.
 */

import { useEffect, useLayoutEffect, useRef, useState } from "react";
import { useNavigate, useParams } from "react-router-dom";
import { api, pictureSet } from "../api";
import type { Credit, Version, Work } from "../api";
import { IdentifyByHand } from "../components/byhand";
import { useSettings } from "../settings";
import { Player } from "../player/player";
import { TrailerPlayer } from "../player/trailer";

export function WorkPage() {
  const { id } = useParams();
  const navigate = useNavigate();
  const { t } = useSettings();
  const [work, setWork] = useState<Work | null>(null);
  const [failed, setFailed] = useState<string | null>(null);
  const [chosen, setChosen] = useState(0);
  const [playing, setPlaying] = useState<{ source: string; fromTheStart: boolean } | null>(null);
  /* A trailer sitting next to the film, which plays from here. One hosted
     elsewhere is watched where it lives instead. */
  const [trailer, setTrailer] = useState<string | null>(null);
  /* Where this viewer stopped, asked for once the page is open rather than
     when play is pressed: the button has to say what it will do before it is
     pressed. */
  const [resumeFrom, setResumeFrom] = useState<number | null>(null);
  /* Counted up when the page has to read the film again, which is what a
     match chosen by hand asks for. */
  const [again, setAgain] = useState(0);

  useEffect(() => {
    if (!id) {
      return;
    }
    const controller = new AbortController();
    setWork(null);
    setFailed(null);
    setChosen(0);
    api
      .work(id, controller.signal)
      .then(setWork)
      .catch((error) => {
        if (!(error instanceof DOMException)) {
          setFailed(error.code === "not_found" ? "error.not_found" : "error.unreachable");
        }
      });
    return () => controller.abort();
  }, [id, again]);

  useEffect(() => {
    const version = work?.versions[chosen];
    if (!version || version.missing) {
      setResumeFrom(null);
      return;
    }
    const controller = new AbortController();
    api
      .plan(version.id, {}, controller.signal)
      .then((plan) => setResumeFrom(plan.resume_from_seconds))
      .catch(() => setResumeFrom(null));
    return () => controller.abort();
  }, [work, chosen]);

  // Escape goes back, which is what a remote control and a keyboard both
  // expect after opening something.
  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        navigate(-1);
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate]);

  if (failed) {
    return (
      <main className="page">
        <p className="notice">{t(failed)}</p>
      </main>
    );
  }
  if (!work) {
    return <main className="page" aria-busy="true" />;
  }

  const backdrop = pictureSet(work.backdrop);
  const poster = pictureSet(work.poster);
  const version = work.versions[chosen];
  const facts = [
    work.year !== null ? String(work.year) : null,
    work.runtime_minutes ? t("work.minutes", { count: work.runtime_minutes }) : null,
    work.age_rating,
    work.rating !== null ? `${work.rating.toFixed(1)} / 10` : null,
  ].filter((fact): fact is string => Boolean(fact));

  if (playing) {
    return (
      <Player
        sourceId={playing.source}
        workId={work.id}
        title={work.title}
        fromTheStart={playing.fromTheStart}
        onClose={() => setPlaying(null)}
      />
    );
  }

  if (trailer) {
    return (
      <TrailerPlayer url={trailer} title={work.title} onClose={() => setTrailer(null)} />
    );
  }

  return (
    <main className="work" style={{ ["--work-color" as string]: work.color ?? "var(--surface)" }}>
      {backdrop && (
        <div className="work-backdrop" aria-hidden="true">
          <img src={backdrop.src} srcSet={backdrop.srcSet} sizes="100vw" alt="" />
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
            />
          ) : (
            <div className="work-poster-empty" aria-hidden="true">
              {work.title.slice(0, 1)}
            </div>
          )}
        </div>

        <div className="work-body">
          <h1 className="work-title">{work.title}</h1>
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
          {id && (
            <IdentifyByHand
              workId={id}
              title={work.title}
              onIdentified={() => setAgain((count) => count + 1)}
            />
          )}

          <div className="work-actions">
            <button
              className="button button-accent button-large"
              disabled={!version || version.missing}
              onClick={() =>
                version && setPlaying({ source: version.id, fromTheStart: false })
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
                  version && setPlaying({ source: version.id, fromTheStart: true })
                }
              >
                {t("player.from_the_start")}
              </button>
            )}
            {/* One sitting next to the film plays here; failing that, a link
                is opened where it lives. This server never goes and fetches
                someone else's video to pass it on. */}
            {(() => {
              const here = work.trailers.find((one) => one.url);
              if (here?.url) {
                return (
                  <button className="button" onClick={() => setTrailer(here.url)}>
                    {t("work.trailer")}
                  </button>
                );
              }
              const elsewhere = work.trailers.find((one) => one.remote_url);
              return elsewhere?.remote_url ? (
                <a
                  className="button"
                  href={elsewhere.remote_url}
                  target="_blank"
                  rel="noreferrer noopener"
                >
                  {t("work.trailer")}
                </a>
              ) : null;
            })()}
          </div>

          <Synopsis text={work.overview} />

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

          {work.external_ids.length > 0 && (
            <p className="work-links">
              <span className="work-studios-label">{t("media.links")}</span>
              {work.external_ids.map((entry) => (
                <a
                  key={entry.provider}
                  className="work-link"
                  href={elsewhere(entry.provider, entry.id)}
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
                  onClick={() => setChosen(index)}
                >
                  {entry.summary || `${t("work.version")} ${index + 1}`}
                </button>
              ))}
            </div>
          )}
          {version && <VersionDetails version={version} />}
        </section>
      )}
    </main>
  );
}

/**
 * The face of one person, or their initial while there is none.
 *
 * The frame is the same either way, so a row of faces where one picture is
 * missing keeps its line rather than shifting everything after it.
 */
function Face({ credit }: { credit: Credit }) {
  const photo = pictureSet(credit.photo);
  if (!photo) {
    return (
      <div className="cast-face" aria-hidden="true">
        {credit.name.slice(0, 1)}
      </div>
    );
  }

  return (
    <div className="cast-face">
      <img src={photo.src} srcSet={photo.srcSet} sizes="96px" alt="" loading="lazy" />
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

function VersionDetails({ version }: { version: Version }) {
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

/** Where a film lives at the site that named it. */
function elsewhere(provider: string, id: string): string {
  switch (provider) {
    case "tmdb":
      return `https://www.themoviedb.org/movie/${id}`;
    case "imdb":
      return `https://www.imdb.com/title/${id}/`;
    default:
      return "";
  }
}

function readableBitrate(bits: number | null): string | null {
  if (bits === null || bits <= 0) {
    return null;
  }
  return bits >= 1_000_000
    ? `${(bits / 1_000_000).toFixed(1)} Mb/s`
    : `${Math.round(bits / 1000)} kb/s`;
}

function readableDate(value: string, language: string): string | null {
  const moment = new Date(value);
  return Number.isNaN(moment.getTime())
    ? null
    : moment.toLocaleString(language, { dateStyle: "medium", timeStyle: "short" });
}

/**
 * Groups the crew so one person credited three times is read once per part,
 * and puts the parts in the order a viewer looks for them: whoever made the
 * film first, and the people a page mentions out of completeness last.
 */
const CREW_ORDER = ["director", "writer", "composer", "producer"];

function groupCrew(crew: { name: string; role: string }[]): [string, string[]][] {
  const grouped = new Map<string, string[]>();
  for (const credit of crew) {
    const names = grouped.get(credit.role) ?? [];
    if (!names.includes(credit.name)) {
      names.push(credit.name);
    }
    grouped.set(credit.role, names);
  }

  return Array.from(grouped.entries()).sort(([left], [right]) => {
    const leftRank = CREW_ORDER.indexOf(left);
    const rightRank = CREW_ORDER.indexOf(right);
    return (leftRank < 0 ? CREW_ORDER.length : leftRank) -
      (rightRank < 0 ? CREW_ORDER.length : rightRank);
  });
}

function readableSize(bytes: number): string {
  const units = ["B", "kB", "MB", "GB", "TB"];
  let value = bytes;
  let unit = 0;
  while (value >= 1000 && unit < units.length - 1) {
    value /= 1000;
    unit += 1;
  }
  return unit === 0 ? `${value} ${units[unit]}` : `${value.toFixed(1)} ${units[unit]}`;
}
