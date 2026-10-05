/*
 * One title found or suggested, as a card with where it stands and what can
 * be done about it: shared by the answers of a search and the suggestions
 * under it.
 */

import type { Found } from "./api";
import { TitleCard, titleAddress } from "./card";
import { StandingBar } from "./standing-bar";

export function Answer({ found, onAsk }: { found: Found; onAsk: () => void }) {
  return (
    <TitleCard
      catalogue={found.catalogue}
      title={found.title}
      year={found.year}
      poster={found.poster}
      overview={found.overview}
      to={titleAddress(found.catalogue, found.tmdb_id)}
    >
      <StandingBar found={found} onAsk={onAsk} />
    </TitleCard>
  );
}
