/*
 * What the page opens on.
 *
 * Five works at the most, shown one at a time and full width. The first is
 * what somebody left halfway, because reaching for the film you were watching
 * is what people come here to do; then what an administrator put in front of
 * everybody, then what has just arrived, then what the server offers. The
 * server decided all that when it filled the row, and says why on each one,
 * so this never works it out a second time.
 *
 * It moves on by itself, slowly, and stops the moment anybody touches it. A
 * banner that carries on sliding while somebody is reading it is a banner
 * that reads its own text out from under them, and one that slides while the
 * pointer is over its buttons is a banner that changes what a click is about
 * to do.
 *
 * It is a module, and the day the home page can be arranged by hand it is one
 * of the ones that can be turned off. Nothing here assumes it is always
 * drawn: an empty one draws nothing at all.
 */

import { useEffect, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { HeroItem } from "../api";
import { useShownPicture } from "./picture";
import { howLong, whatTheFileHolds, whichEpisode } from "../readable";
import { useSettings } from "../settings";
import { ChevronLeftIcon, ChevronRightIcon, InfoIcon, PlayIcon } from "../icons";

/** How many genres are named beside a title. Two: past that it is a list of
 *  everything the film could be filed under rather than what it is. */
const GENRES_NAMED = 2;

/** How long one work stands before the next takes its place. */
const EACH_STANDS_FOR = 9000;

export function Hero({ items }: { items: HeroItem[] }) {
  const { t } = useSettings();
  const [at, setAt] = useState(0);
  /* Set by the first touch of any kind and never unset: a banner that starts
     moving again the moment somebody looks away is worse than one that never
     moved, because it moves exactly when nobody is watching for it. */
  const [held, setHeld] = useState(false);
  const holder = useRef<HTMLElement>(null);

  const many = items.length > 1;

  useEffect(() => {
    if (!many || held) {
      return;
    }
    const step = window.setInterval(
      () => setAt((was) => (was + 1) % items.length),
      EACH_STANDS_FOR,
    );
    return () => window.clearInterval(step);
  }, [many, held, items.length]);

  // Somebody who asked their system for less movement is not shown a banner
  // that moves on by itself at all.
  useEffect(() => {
    const asked = window.matchMedia("(prefers-reduced-motion: reduce)");
    if (asked.matches) {
      setHeld(true);
    }
  }, []);

  if (items.length === 0) {
    return null;
  }

  const shown = items[Math.min(at, items.length - 1)];
  const go = (to: number) => {
    setHeld(true);
    setAt((to + items.length) % items.length);
  };

  return (
    <section
      className="hero"
      ref={holder}
      aria-label={t("home.hero")}
      onPointerEnter={() => setHeld(true)}
      onFocusCapture={() => setHeld(true)}
    >
      {/* Every one of them is drawn, and only the one in front is shown: the
          pictures are then already in the browser when the banner moves on,
          so it changes rather than blinking through a hole. */}
      {items.map((item, rank) => (
        <HeroBackdrop key={item.id} item={item} shown={rank === at} />
      ))}

      <div className="hero-words">
        {shown.tagline && <p className="hero-tagline">{shown.tagline}</p>}
        <HeroTitle item={shown} />

        <p className="hero-facts">
          {[
            // An episode leads with which episode it is: the title above is
            // its series, so without this nobody knows where they left off.
            whichEpisode(shown, t),
            shown.year,
            shown.runtime_minutes ? howLong(shown.runtime_minutes, t) : null,
            ...shown.genres.slice(0, GENRES_NAMED),
            shown.episodes > 0 ? t("card.unwatched", { count: shown.unwatched }) : null,
          ]
            .filter(Boolean)
            .join(" · ")}
        </p>

        {/* What the file itself holds. Beside the title because it is what
            decides whether somebody watches this one here or on the screen
            in the other room, and because it is the one thing a catalogue
            cannot promise: it was read off the copy on the disk. */}
        <HeroBadges item={shown} />

        {shown.overview && <p className="hero-overview">{shown.overview}</p>}

        <div className="hero-buttons">
          <HeroPlay item={shown} />
          <Link className="button button-large" to={`/work/${shown.id}`}>
            <InfoIcon size={19} />
            {t("home.hero.open")}
          </Link>
        </div>

        {/* How far in it already is, under the buttons that carry on with it. */}
        <HeroProgress item={shown} />
      </div>

      {many && (
        <>
          <button
            type="button"
            className="hero-step hero-step-back"
            aria-label={t("home.hero.previous")}
            onClick={() => go(at - 1)}
          >
            <ChevronLeftIcon size={32} />
          </button>
          <button
            type="button"
            className="hero-step hero-step-on"
            aria-label={t("home.hero.next")}
            onClick={() => go(at + 1)}
          >
            <ChevronRightIcon size={32} />
          </button>

          <div className="hero-dots">
            {items.map((item, rank) => (
              <button
                key={item.id}
                type="button"
                className={`hero-dot${rank === at ? " hero-dot-on" : ""}`}
                aria-label={item.title}
                aria-current={rank === at}
                onClick={() => go(rank)}
              />
            ))}
          </div>
        </>
      )}
    </section>
  );
}

/** What the copy on the disk holds, as badges. Nothing at all for a work
 *  whose file nobody has analysed, rather than a row of empty pills. */
function HeroBadges({ item }: { item: HeroItem }) {
  const badges = whatTheFileHolds(item);

  if (badges.length === 0) {
    return null;
  }
  return (
    <p className="hero-badges">
      {badges.map((badge) => (
        <span className="hero-badge" key={badge}>
          {badge}
        </span>
      ))}
    </p>
  );
}

/**
 * How far in it already is.
 *
 * Where it stopped and how long it is, both spelled out: "one hour twelve of
 * two hours forty six" says at once how much was watched and how much is
 * left, which a bar alone never says and a count of minutes left says only
 * half of.
 */
function HeroProgress({ item }: { item: HeroItem }) {
  const { t } = useSettings();

  if (item.resume_from_seconds === null || !item.runtime_minutes) {
    return null;
  }
  const whole = item.runtime_minutes * 60;
  const done = Math.min(item.resume_from_seconds, whole);

  return (
    <p className="hero-progress">
      <span className="hero-progress-bar" aria-hidden="true">
        <span className="hero-progress-done" style={{ width: `${(done / whole) * 100}%` }} />
      </span>
      <span className="hero-progress-said">
        {t("home.hero.progress", {
          done: howLong(Math.round(done / 60), t),
          whole: howLong(item.runtime_minutes, t),
        })}
      </span>
    </p>
  );
}

/** The wide picture behind one work, or its own colour when it has none. */
function HeroBackdrop({ item, shown }: { item: HeroItem; shown: boolean }) {
  const { picture, itDidNotLoad } = useShownPicture(item.backdrop);

  return (
    <div
      className={`hero-backdrop${shown ? " hero-backdrop-shown" : ""}`}
      style={{ ["--card-color" as string]: item.color ?? "var(--surface-raised)" }}
      aria-hidden={!shown}
    >
      {picture && (
        <img
          src={picture.src}
          srcSet={picture.srcSet}
          sizes="100vw"
          alt=""
          decoding="async"
          onError={itDidNotLoad}
        />
      )}
    </div>
  );
}

/** Its title as its own designers drew it, or written out when there is
 *  none: a work nobody has looked up still has a name. An episode is named
 *  by its series, which is the only name anybody remembers it by. */
function HeroTitle({ item }: { item: HeroItem }) {
  const { picture, itDidNotLoad } = useShownPicture(item.logo);
  const name = item.series_title ?? item.title;

  if (!picture) {
    return <h2 className="hero-title">{name}</h2>;
  }
  return (
    <h2 className="hero-title hero-title-drawn">
      <img
        src={picture.src}
        srcSet={picture.srcSet}
        alt={name}
        decoding="async"
        onError={itDidNotLoad}
      />
    </h2>
  );
}

/** Carry on, or start: what the button says comes from why the server put
 *  this work here, so the two never disagree. */
function HeroPlay({ item }: { item: HeroItem }) {
  const { t } = useSettings();
  const navigate = useNavigate();

  if (item.source === null || item.kind === "series") {
    return (
      <Link className="button button-accent button-large" to={`/work/${item.id}`}>
        <PlayIcon size={20} />
        {t("home.hero.open")}
      </Link>
    );
  }
  return (
    <button
      type="button"
      className="button button-accent button-large"
      onClick={() => navigate(`/work/${item.id}?play`)}
    >
      <PlayIcon size={20} />
      {t(item.because === "started" ? "home.hero.carry_on" : "work.play")}
    </button>
  );
}
