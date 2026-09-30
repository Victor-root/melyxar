/*
 * The server's collections: every one of them with a poster, and the page of
 * one with its titles in its order.
 *
 * Both read again whenever a title is put in or taken out of a collection,
 * wherever that was done.
 */

import { useParams } from "react-router-dom";
import { api } from "../api";
import { useAccount } from "../account";
import { refusalOf, useAsked } from "../asking";
import { PageBackdrop } from "../components/backdrop";
import { Card } from "../components/card";
import { Grid } from "../components/grid";
import { InOrder } from "../components/in-order";
import { ListHead, ListTile } from "../components/lists";
import { refusalKey } from "../i18n";
import { useLeave } from "../leaving";
import { useMarks } from "../marks";
import { useSettings } from "../settings";

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
        {asked.answer?.length === 0 && <p className="notice">{t("collections.none")}</p>}
        {asked.answer && asked.answer.length > 0 && (
          <Grid>
            {asked.answer.map((collection) => (
              <ListTile
                key={collection.id}
                id={collection.id}
                name={collection.name}
                count={collection.count}
                cover={collection.cover}
                to={`/collection/${collection.id}`}
              />
            ))}
          </Grid>
        )}
      </main>
    </>
  );
}

export function CollectionPage() {
  const { id } = useParams();
  const { t } = useSettings();
  const leave = useLeave("/collections");
  const { account } = useAccount();
  const { rowsMoved, rowsHaveMoved } = useMarks();
  const asked = useAsked(
    (signal) => (id ? api.collection(id, signal) : Promise.resolve(null)),
    [id, rowsMoved],
    id ? `collection:${id}` : undefined,
  );
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

  return (
    <>
      <PageBackdrop />
      <main className="page">
        <ListHead
          name={collection.name}
          cards={collection.cards}
          words="collections"
          onRename={
            managed
              ? async (name) => {
                  await api.renameCollection(collection.id, name);
                  rowsHaveMoved();
                }
              : null
          }
          onDelete={
            managed
              ? async () => {
                  await api.deleteCollection(collection.id);
                  rowsHaveMoved();
                  leave();
                }
              : null
          }
        />
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
    </>
  );
}
