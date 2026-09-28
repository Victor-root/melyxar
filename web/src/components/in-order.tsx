/*
 * The cards of a row or a grid, in the order they are shown, for what one of
 * them does with the ones after it: playing everything from there on.
 *
 * Read only by a card's menu, which is drawn when opened, so the cards of the
 * list are never drawn again for it.
 */

import { createContext, useContext } from "react";
import type { ReactNode } from "react";
import type { Card } from "../api";

const CardsInOrder = createContext<Card[] | null>(null);

export function InOrder({ cards, children }: { cards: Card[]; children: ReactNode }) {
  return <CardsInOrder.Provider value={cards}>{children}</CardsInOrder.Provider>;
}

/** The cards of the list this one is shown in, or nothing outside a list. */
export function useCardsInOrder(): Card[] | null {
  return useContext(CardsInOrder);
}
