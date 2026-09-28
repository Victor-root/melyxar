/*
 * What IMDb's viewers and Rotten Tomatoes' critics made of a work, beside
 * its provider's own rating on the line of facts of its page.
 */

import type { Rating } from "../api";
import { SplatIcon, TomatoIcon } from "../icons";
import { howManyVoted, outOfTen, percentOf } from "../readable";
import { useSettings } from "../settings";

/** The share of critics from which Rotten Tomatoes calls a work fresh. */
const FRESH_FROM = 60;

export function ElsewhereRatings({ ratings }: { ratings: Rating[] }) {
  const { t, language } = useSettings();
  return (
    <>
      {ratings.map((rating) => {
        if (rating.source === "imdb") {
          const said = t("rating.imdb", {
            value: outOfTen(rating.value),
            votes: howManyVoted(rating.votes ?? 0, language),
          });
          return (
            <span key={rating.source} className="work-rating" title={said} aria-label={said}>
              <span className="work-rating-mark" aria-hidden="true">
                IMDb
              </span>
              {outOfTen(rating.value)}
            </span>
          );
        }
        const share = percentOf(rating.value / 100, language);
        const said = t("rating.critics", { share });
        return (
          <span key={rating.source} className="work-rating" title={said} aria-label={said}>
            {rating.value >= FRESH_FROM ? <TomatoIcon size={17} /> : <SplatIcon size={17} />}
            {share}
          </span>
        );
      })}
    </>
  );
}
