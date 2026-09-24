/*
 * What somebody filmed or photographed themselves: a folder, and a photo.
 *
 * Neither looks like the page of a film. A folder is what it holds, laid out
 * as wide tiles because what is filmed and photographed is mostly wider than
 * tall. A photo is the photo, as large as the window allows, with the way to
 * the next one under the hand.
 */

import { useEffect } from "react";
import { Link, useNavigate } from "react-router-dom";
import type { Child, Work } from "../api";
import { WayBackUp } from "../components/ancestry";
import { DeleteButton } from "../components/deletion";
import { SelectMark, Selecting, useChoosingPress } from "../components/selection";
import { useShownPicture } from "../components/picture";
import { ChevronLeftIcon, ChevronRightIcon, HomeMediaIcon } from "../icons";
import { useMarks } from "../marks";
import { howMany } from "../readable";
import { useSettings } from "../settings";

/** A folder of one's own: its folders first, then its videos and photos. */
export function FolderView({ work }: { work: Work }) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const marks = useMarks();
  const children = work.children.filter((child) => !marks.goneOf(child.card.id));

  return (
    <main className="page own-folder">
      <WayBackUp work={work} />
      <div className="section-head">
        <h1>{work.title}</h1>
        <span className="count">{howMany(children.length, "own.item_count", t)}</span>
        <DeleteButton workId={work.id} title={work.title} onDeleted={() => navigate(-1)} />
      </div>
      {children.length === 0 ? (
        <p className="notice">{t("own.empty")}</p>
      ) : (
        <Selecting items={children.map((child) => child.card)}>
          <div className="own-grid">
            {children.map((child) => (
              <OwnTile key={child.card.id} child={child} />
            ))}
          </div>
        </Selecting>
      )}
    </main>
  );
}

/** One thing a folder holds. A video starts playing at once, which is what
 *  somebody opening a video of their own wants; a folder and a photo open. */
function OwnTile({ child }: { child: Child }) {
  const { t } = useSettings();
  const { card } = child;
  const { picture, itDidNotLoad } = useShownPicture(card.poster);
  const playable = card.source !== null;
  const to = card.kind === "video" && playable ? `/work/${card.id}?play` : `/work/${card.id}`;
  const choosing = useChoosingPress(card.id);

  return (
    <div
      className={`own-cell${choosing.selecting ? " selecting" : ""}${choosing.chosen ? " own-chosen" : ""}`}
    >
      <Link
        to={to}
        className={`own-tile own-tile-${card.kind}`}
        style={{ ["--card-color" as string]: card.color ?? "var(--surface)" }}
        onClick={choosing.onClick}
      >
        <div className="own-picture">
          {picture ? (
            <img
              src={picture.src}
              srcSet={picture.srcSet}
              sizes="(max-width: 800px) 45vw, 280px"
              alt=""
              loading="lazy"
              onError={itDidNotLoad}
            />
          ) : (
            <div className="own-picture-empty" aria-hidden="true">
              <HomeMediaIcon size={32} />
            </div>
          )}
          {card.kind === "folder" && (
            <span className="own-badge">{howMany(child.child_count, "own.item_count", t)}</span>
          )}
          {card.kind === "video" && (
            <span className="own-badge">
              <span className="play-mark" aria-hidden="true" />
              {card.runtime_minutes ? t("work.minutes", { count: card.runtime_minutes }) : ""}
            </span>
          )}
        </div>
        <span className="own-name">{card.title}</span>
        {!playable && card.kind !== "folder" && (
          <span className="own-missing">{t("work.not_on_disk")}</span>
        )}
      </Link>
      {/* Beside the link rather than in it, laid over the corner of the
          picture: a button inside a link is a press that goes two ways. */}
      <div className="own-select">
        <SelectMark id={card.id} />
      </div>
    </div>
  );
}

/**
 * A photo, whole.
 *
 * The arrows of the keyboard and the two buttons step through the folder. The
 * step replaces the page rather than stacking one on top of another, so that
 * going back after looking through fifty photos leads to the folder and not to
 * the forty ninth.
 */
export function PhotoView({ work }: { work: Work }) {
  const { t } = useSettings();
  const navigate = useNavigate();
  const { previous_photo: previous, next_photo: next } = work;
  const missing = work.versions.every((version) => version.missing);

  useEffect(() => {
    const onKey = (event: KeyboardEvent) => {
      // Not while a panel is open over the photo: its arrows are its own.
      if (event.target instanceof Element && event.target.closest("[role=dialog]")) {
        return;
      }
      const towards = event.key === "ArrowLeft" ? previous : event.key === "ArrowRight" ? next : null;
      if (towards) {
        navigate(`/work/${towards}`, { replace: true });
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [navigate, previous, next]);

  return (
    <main className="own-photo">
      <div className="own-photo-head">
        <WayBackUp work={work} />
        <h1 className="own-photo-title">{work.title}</h1>
        {/* Gone, the photo gives way to the one after it, or the one before,
            and to its folder when it was the last. */}
        <DeleteButton
          workId={work.id}
          title={work.title}
          onDeleted={() => {
            const instead = next ?? previous;
            if (instead) {
              navigate(`/work/${instead}`, { replace: true });
            } else {
              navigate(-1);
            }
          }}
        />
      </div>
      <div className="own-photo-stage">
        {missing ? (
          <p className="notice">{t("work.not_on_disk")}</p>
        ) : (
          <img className="own-photo-image" src={`/api/v1/works/${work.id}/photo`} alt={work.title} />
        )}
        {previous && (
          <Link
            className="own-photo-step own-photo-previous"
            to={`/work/${previous}`}
            replace
            aria-label={t("own.previous_photo")}
            title={t("own.previous_photo")}
          >
            <ChevronLeftIcon size={28} />
          </Link>
        )}
        {next && (
          <Link
            className="own-photo-step own-photo-next"
            to={`/work/${next}`}
            replace
            aria-label={t("own.next_photo")}
            title={t("own.next_photo")}
          >
            <ChevronRightIcon size={28} />
          </Link>
        )}
      </div>
    </main>
  );
}
