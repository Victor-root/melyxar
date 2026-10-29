import { describe, expect, it } from "vitest";
import type { ImportPreview } from "../../api";
import { ALL_PARTS, choicesOf, firstCollections, firstPlans, readyToTakeUp } from "./import-choices";

const preview: ImportPreview = {
  server: "emby",
  oldest_read: "4.8",
  accounts: [
    { key: "1", name: "Ada", administrator: true, disabled: false, here: { id: "u-ada", name: "ada" } },
    { key: "2", name: "Ben", administrator: false, disabled: false, here: null },
    { key: "3", name: "Old", administrator: false, disabled: true, here: null },
  ],
  found: [],
  taken: { watched: 0, left_partway: 0, favourites: 0, listens: 0 },
  playlists: [],
  collections: [
    { name: "Rainy days", items: 2, found: 2, automatic: false },
    { name: "Saga", items: 3, found: 3, automatic: true },
    { name: "Unknown", items: 1, found: 1, automatic: null },
  ],
  lost: [],
  lost_count: 0,
};

describe("firstPlans", () => {
  it("joins the account of the same name, makes one otherwise, and leaves one switched off", () => {
    const plans = firstPlans(preview);
    expect(plans["1"]).toEqual({ becomes: "existing", user: "u-ada", name: "Ada", password: "" });
    expect(plans["2"].becomes).toBe("new");
    expect(plans["3"].becomes).toBe("nothing");
  });
});

describe("firstCollections", () => {
  it("offers every collection but the ones the other server made itself", () => {
    expect([...firstCollections(preview)]).toEqual(["Rainy days", "Unknown"]);
  });
});

describe("readyToTakeUp and choicesOf", () => {
  it("waits for the password of every account made, then sends each choice", () => {
    const plans = firstPlans(preview);
    expect(readyToTakeUp(plans)).toBe(false);
    plans["2"] = { ...plans["2"], name: " Ben ", password: "quiet harbour" };
    expect(readyToTakeUp(plans)).toBe(true);
    const parts = { ...ALL_PARTS, favourites: false };
    expect(choicesOf(plans, new Set(["Rainy days"]), parts)).toEqual({
      accounts: [
        { becomes: "existing", key: "1", user: "u-ada" },
        { becomes: "new", key: "2", name: "Ben", password: "quiet harbour" },
        { becomes: "nothing", key: "3" },
      ],
      collections: ["Rainy days"],
      parts,
    });
  });

  it("has nothing to take up when every account is left aside", () => {
    const plans = firstPlans(preview);
    for (const plan of Object.values(plans)) {
      plan.becomes = "nothing";
    }
    expect(readyToTakeUp(plans)).toBe(false);
  });
});
