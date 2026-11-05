/*
 * This page as a device the account's remotes steer: a film or an episode
 * started here from another device, opened and played as if it had been
 * pressed here, and what the page says of itself said again whenever its
 * line opens. What plays answers the other orders itself (`player/steered.ts`
 * and `music/player/remote.ts`).
 */

import { useNavigate } from "react-router-dom";
import { useRemoteOrders } from "../live";
import { playingAddress } from "../screens/work";
import { useToldAgainOnOpening } from "./told";

export function SteeredHere() {
  const navigate = useNavigate();
  useToldAgainOnOpening();
  useRemoteOrders((order) => {
    if (order.do === "play_work") {
      navigate(playingAddress(order.work, order.source));
    }
  });
  return null;
}
