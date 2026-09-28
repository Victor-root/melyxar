/*
 * This account's playlists: every one of them with a poster, and the page of
 * one, where its titles are put in order by dragging them and taken out one
 * by one.
 *
 * The order and what is taken out show at once and go back as they were if
 * the server refuses; the other screens showing the playlist read it again
 * once it agrees.
 */

import { useEffect, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import type { Card as CardData } from "../api";
import { refusalOf, useAsked } from "../asking";
import { PageBackdrop } from "../components/backdrop";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { ListHead, ListTile } from "../components/lists";
import { Sortable } from "../components/sortable";
import { refusalKey } from "../i18n";
import { CloseIcon } from "../icons";
import { useMarks } from "../marks";
import { howLong } from "../readable";
import { useSettings } from "../settings";

export function PlaylistsPage() {
  const { t } = useSettings();
  const { rowsMoved } = useMarks();
  const asked = useAsked((signal) => api.playlists(signal), [rowsMoved], "playlists");

  return (
    <>
      <PageBackdrop />
      <main className="page">
        <div className="section-head">
          <h1>{t("nav.playlists")}</h1>
        </div>
        {asked.failure && <p className="notice">{t(refusalKey(refusalOf(asked.failure)))}</p>}
        {asked.answer?.length === 0 && <p className="notice">{t("playlists.none")}</p>}
        {asked.answer && asked.answer.length > 0 && (
          <Grid>
            {asked.answer.map((playlist) => (
              <ListTile
                key={playlist.id}
                id={playlist.id}
                name={playlist.name}
                count={playlist.count}
                cover={playlist.cover}
                to={`/playlist/${playlist.id}`}
              />
            ))}
          </Grid>
        )}
      </main>
    </>
  );
}

export function PlaylistPage() {
  const { id } = useParams();
  const { t } = useSettings();
  const navigate = useNavigate();
  const { rowsMoved, rowsHaveMoved } = useMarks();
  const asked = useAsked(
    (signal) => (id ? api.playlist(id, signal) : Promise.resolve(null)),
    [id, rowsMoved],
    id ? `playlist:${id}` : undefined,
  );
  const playlist = asked.answer;
  /* The titles as this page holds them: moved and taken out at once, and
     put back if the server refuses. */
  const [cards, setCards] = useState<CardData[]>([]);
  const [refused, setRefused] = useState<string | null>(null);
  useEffect(() => {
    if (playlist) {
      setCards(playlist.cards);
    }
  }, [playlist]);

  if (asked.failure) {
    return (
      <main className="page">
        <p className="notice">
          {t(asked.failure.code === "not_found" ? "error.not_found" : "error.unreachable")}
        </p>
      </main>
    );
  }
  if (!playlist || !id) {
    return <main className="page" aria-busy="true" />;
  }

  const change = async (next: CardData[], asking: () => Promise<unknown>) => {
    const before = cards;
    setCards(next);
    setRefused(null);
    try {
      await asking();
      rowsHaveMoved();
    } catch (error) {
      setCards(before);
      setRefused(refusalOf(error));
    }
  };

  return (
    <>
      <PageBackdrop />
      <main className="page">
        <ListHead
          name={playlist.name}
          cards={cards}
          words="playlists"
          onRename={async (name) => {
            await api.renamePlaylist(id, name);
            rowsHaveMoved();
          }}
          onDelete={async () => {
            await api.deletePlaylist(id);
            rowsHaveMoved();
            navigate("/playlists", { replace: true });
          }}
        />
        {refused && <p className="notice">{t(refusalKey(refused))}</p>}
        {cards.length === 0 ? (
          <p className="notice">{t("playlists.empty")}</p>
        ) : (
          <>
            <p className="settings-why">{t("playlists.drag")}</p>
            <div className="playlist-lines">
              <Sortable
                items={cards}
                keyOf={(card) => card.id}
                nameOf={(card) => card.title}
                onMove={(moved) =>
                  void change(moved, () =>
                    api.reorderPlaylist(
                      id,
                      moved.map((card) => card.id),
                    ),
                  )
                }
              >
                {(card) => (
                  <div className="playlist-line">
                    <Card card={card} shape="lying" named={false} cornered={false} />
                    <div className="playlist-line-words">
                      <Link className="playlist-line-name" to={`/work/${card.id}`}>
                        {card.title}
                      </Link>
                      <span className="playlist-line-facts">
                        {[card.year, card.runtime_minutes ? howLong(card.runtime_minutes, t) : null]
                          .filter(Boolean)
                          .join(" · ")}
                      </span>
                    </div>
                    <button
                      type="button"
                      className="playlist-line-off"
                      aria-label={t("playlists.take_out", { name: card.title })}
                      title={t("playlists.take_out", { name: card.title })}
                      onClick={() =>
                        void change(
                          cards.filter((held) => held.id !== card.id),
                          () => api.putInPlaylist(id, [card.id], false),
                        )
                      }
                    >
                      <CloseIcon size={16} />
                    </button>
                  </div>
                )}
              </Sortable>
            </div>
          </>
        )}
      </main>
    </>
  );
}
