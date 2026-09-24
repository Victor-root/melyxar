/*
 * One person credited on a work, as a card in a row.
 *
 * Drawn as the cards of the home page are drawn, standing, the face in place
 * of the poster, so a row of the cast moves, grows under the pointer and is
 * stepped through the same way every other row is. The whole card opens the
 * person's own page.
 */

import { Link } from "react-router-dom";
import type { Credit } from "../api";
import { useShownPicture } from "./picture";

export function PersonCard({ credit }: { credit: Credit }) {
  const { picture: photo, itDidNotLoad } = useShownPicture(credit.photo);

  return (
    <article className="card card-standing card-person" data-card>
      <div className="card-picture">
        {photo ? (
          <img
            src={photo.src}
            srcSet={photo.srcSet}
            /* The widths a standing card is drawn at, written out for the
               reason the cards of the home page write theirs. */
            sizes="(max-width: 700px) 40vw, (min-width: 1400px) 186px, 168px"
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onError={itDidNotLoad}
          />
        ) : (
          <span className="card-initial" aria-hidden="true">
            {credit.name.slice(0, 1)}
          </span>
        )}
        <Link
          className="card-open"
          to={`/person/${credit.person_id}`}
          title={credit.name}
          draggable={false}
        >
          <span className="visually-hidden">{credit.name}</span>
        </Link>
      </div>
      <span className="card-line">
        <span className="card-title">{credit.name}</span>
      </span>
      <span className="card-year">{credit.character ?? ""}</span>
    </article>
  );
}
