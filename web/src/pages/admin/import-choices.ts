/*
 * What the administrator chooses for each account of an Emby or Jellyfin
 * backup, before it is taken up: what is suggested first, what is sent, and
 * whether it can be sent yet.
 */

import type { ImportAccountChoice, ImportChoices, ImportPreview } from "../../api";

/** What becomes of one account over there, as the screen holds it. */
export interface AccountPlan {
  becomes: "existing" | "new" | "nothing";
  /** The account here, when it becomes one that exists. */
  user: string;
  /** The name and password of the account made, when one is made. */
  name: string;
  password: string;
}

/** An account becomes the one here of the same name; failing one, an account
 *  is made under its name, unless it was switched off over there. */
export function firstPlans(preview: ImportPreview): Record<string, AccountPlan> {
  return Object.fromEntries(
    preview.accounts.map((account) => [
      account.key,
      {
        becomes: account.here ? "existing" : account.disabled ? "nothing" : "new",
        user: account.here?.id ?? "",
        name: account.name,
        password: "",
      },
    ]),
  );
}

/** Every collection but the ones the other server made itself; a server
 *  that cannot tell has them all offered. */
export function firstCollections(preview: ImportPreview): Set<string> {
  return new Set(preview.collections.filter((collection) => collection.automatic !== true).map((c) => c.name));
}

export function choicesOf(plans: Record<string, AccountPlan>, collections: Set<string>): ImportChoices {
  const accounts: ImportAccountChoice[] = Object.entries(plans).map(([key, plan]) => {
    switch (plan.becomes) {
      case "existing":
        return { becomes: "existing", key, user: plan.user };
      case "new":
        return { becomes: "new", key, name: plan.name.trim(), password: plan.password };
      default:
        return { becomes: "nothing", key };
    }
  });
  return { accounts, collections: [...collections] };
}

/** Whether there is something to take up, and every account made has a name
 *  and a password and every account joined is one. */
export function readyToTakeUp(plans: Record<string, AccountPlan>): boolean {
  const all = Object.values(plans);
  return (
    all.some((plan) => plan.becomes !== "nothing") &&
    all.every(
      (plan) =>
        plan.becomes === "nothing" ||
        (plan.becomes === "existing" && plan.user !== "") ||
        (plan.becomes === "new" && plan.name.trim() !== "" && plan.password !== ""),
    )
  );
}
