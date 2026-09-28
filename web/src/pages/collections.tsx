/*
 * The server's collections: every one of them with a poster, and the page of
 * one with its titles in its order.
 *
 * Both read again whenever a title is put in or taken out of a collection,
 * wherever that was done.
 */

import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { api } from "../api";
import type { CollectionSummary } from "../api";
import { useAccount } from "../account";
import { refusalOf, useAsked } from "../asking";
import { PageBackdrop } from "../components/backdrop";
import { Card, ROOM_FOR_A_PICTURE } from "../components/card";
import { Grid } from "../components/grid";
import { InOrder } from "../components/in-order";
import { Modal } from "../components/modal";
import { useShownPicture } from "../components/picture";
import { refusalKey } from "../i18n";
import { DeleteIcon, EditIcon, PlayAllIcon } from "../icons";
import { useMarks } from "../marks";
import { playInTurn } from "../queue";
import { howMany } from "../readable";
import { useSettings } from "../settings";
import { playsOnItsOwn } from "../works";

export function CollectionsPage() {
  const { t } = useSettings();
  const { rowsMoved } = useMarks();
  const asked = useAsked((signal) => api.collections(signal), [rowsMoved], "collections");

  return (
    <>
      <PageBackdrop />
      <main className="page">
        <div className="section-head">
          <h1>{t("nav.collections")}</h1>
        </div>
        {asked.failure && <p className="notice">{t(refusalKey(refusalOf(asked.failure)))}</p>}
        {asked.answer && asked.answer.length === 0 && (
          <p className="notice">{t("collections.none")}</p>
        )}
        {asked.answer && asked.answer.length > 0 && (
          <Grid>
            {asked.answer.map((collection) => (
              <CollectionTile key={collection.id} collection={collection} />
            ))}
          </Grid>
        )}
      </main>
    </>
  );
}

/** One collection as a card: the poster of its first title, its name, and
 *  how many titles it holds. */
function CollectionTile({ collection }: { collection: CollectionSummary }) {
  const { t } = useSettings();
  const { picture, itDidNotLoad } = useShownPicture(collection.cover?.poster ?? []);
  return (
    <article
      className="card card-standing"
      data-card={collection.id}
      style={{ ["--card-color" as string]: collection.cover?.color ?? "var(--surface-raised)" }}
    >
      <div className="card-picture">
        {picture ? (
          <img
            src={picture.src}
            srcSet={picture.srcSet}
            sizes={ROOM_FOR_A_PICTURE.standing}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {collection.name.slice(0, 1)}
          </span>
        )}
        <Link className="card-open" to={`/collection/${collection.id}`} title={collection.name} draggable={false}>
          <span className="visually-hidden">{collection.name}</span>
        </Link>
      </div>
      <span className="card-line">
        <span className="card-title">{collection.name}</span>
      </span>
      <span className="card-year">{howMany(collection.count, "collections.count", t)}</span>
    </article>
  );
}

export function CollectionPage() {
  const { id } = useParams();
  const { t } = useSettings();
  const navigate = useNavigate();
  const { account } = useAccount();
  const { rowsMoved, rowsHaveMoved } = useMarks();
  const asked = useAsked(
    (signal) => (id ? api.collection(id, signal) : Promise.resolve(null)),
    [id, rowsMoved],
    id ? `collection:${id}` : undefined,
  );
  const [renaming, setRenaming] = useState<string | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [refused, setRefused] = useState<string | null>(null);
  const collection = asked.answer;

  if (asked.failure) {
    return (
      <main className="page">
        <p className="notice">
          {t(asked.failure.code === "not_found" ? "error.not_found" : "error.unreachable")}
        </p>
      </main>
    );
  }
  if (!collection) {
    return <main className="page" aria-busy="true" />;
  }

  const managed = collection.made_by_hand && account?.may_manage_collections === true;
  const playable = collection.cards.filter(playsOnItsOwn).map((card) => card.id);

  const playAll = () => {
    playInTurn(playable);
    navigate(`/work/${playable[0]}?play`);
  };

  const rename = async (name: string) => {
    setRefused(null);
    try {
      await api.renameCollection(collection.id, name);
      setRenaming(null);
      rowsHaveMoved();
    } catch (error) {
      setRefused(refusalOf(error));
    }
  };

  const remove = async () => {
    try {
      await api.deleteCollection(collection.id);
      rowsHaveMoved();
      navigate("/collections", { replace: true });
    } catch (error) {
      setDeleting(false);
      setRefused(refusalOf(error));
    }
  };

  return (
    <>
      <PageBackdrop />
      <main className="page">
        <div className="section-head collection-head">
          {renaming === null ? (
            <h1>{collection.name}</h1>
          ) : (
            <form
              className="collect-new collection-rename"
              onSubmit={(event) => {
                event.preventDefault();
                void rename(renaming);
              }}
            >
              <input
                type="text"
                className="field-line"
                aria-label={t("collections.name")}
                autoFocus
                maxLength={80}
                value={renaming}
                onChange={(event) => setRenaming(event.target.value)}
              />
              <button type="submit" className="button button-small button-accent" disabled={renaming.trim() === ""}>
                {t("collections.keep")}
              </button>
              <button type="button" className="button button-small button-quiet" onClick={() => setRenaming(null)}>
                {t("collections.cancel")}
              </button>
            </form>
          )}
          <span className="collection-count">{howMany(collection.cards.length, "collections.count", t)}</span>
          <span className="collection-actions">
            {playable.length > 0 && (
              <button type="button" className="button button-small button-accent" onClick={playAll}>
                <PlayAllIcon size={16} />
                {t("collections.play_all")}
              </button>
            )}
            {managed && renaming === null && (
              <button type="button" className="button button-small" onClick={() => setRenaming(collection.name)}>
                <EditIcon size={15} />
                {t("collections.rename")}
              </button>
            )}
            {managed && (
              <button type="button" className="button button-small button-quiet" onClick={() => setDeleting(true)}>
                <DeleteIcon size={15} />
                {t("collections.delete")}
              </button>
            )}
          </span>
        </div>
        {refused && <p className="notice">{t(refusalKey(refused))}</p>}
        {collection.cards.length === 0 ? (
          <p className="notice">{t("collections.empty")}</p>
        ) : (
          <InOrder cards={collection.cards}>
            <Grid>
              {collection.cards.map((card) => (
                <Card key={card.id} card={card} />
              ))}
            </Grid>
          </InOrder>
        )}
      </main>
      {deleting && (
        <Modal
          title={t("collections.delete_title", { name: collection.name })}
          onClose={() => setDeleting(false)}
          footer={
            <button className="button button-accent" onClick={() => void remove()}>
              {t("collections.delete")}
            </button>
          }
        >
          <p>{t("collections.delete_why")}</p>
        </Modal>
      )}
    </>
  );
}
