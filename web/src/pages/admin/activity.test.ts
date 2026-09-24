import { describe, expect, it } from "vitest";
import type { ActivityLine } from "../../api";
import { familyOf, lastingFor, sayLine, sayPoint, whenItHappened } from "./activity";

/** Standing in for the wording, so what is checked is what goes into it. */
const t = (key: string, values?: Record<string, string | number>) =>
  values ? `${key}(${Object.values(values).join("|")})` : key;

function line(kind: string, details: Record<string, unknown>, device: string | null = null): ActivityLine {
  return {
    id: "l",
    at: "2026-01-01T20:00:00Z",
    kind,
    level: "information",
    user_id: null,
    work_id: null,
    device,
    details,
  };
}

const WINDOWS_CHROME =
  "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/140.0.0.0 Safari/537.36";

describe("familyOf", () => {
  it("files every kind under the family it is filtered by", () => {
    expect(familyOf("watched")).toBe("playback");
    expect(familyOf("task_failed")).toBe("library");
    expect(familyOf("works_deleted")).toBe("library");
    expect(familyOf("server_started")).toBe("server");
    expect(familyOf("sign_in_refused")).toBe("access");
  });
});

describe("lastingFor", () => {
  it("counts seconds under a minute, and minutes and hours above", () => {
    expect(lastingFor(42.4, t)).toBe("activity.seconds(42)");
    expect(lastingFor(1500, t)).toBe("work.minutes(25)");
    expect(lastingFor(5400, t)).toBe("work.hours_minutes(1|30)");
  });
});

describe("sayLine", () => {
  it("names an episode by its series and where it sits, and says how it was watched", () => {
    const said = sayLine(
      line(
        "watched",
        {
          user_name: "somebody",
          title: "The Long Night",
          series: "Lantern Street",
          season: 2,
          episode: 5,
          played_seconds: 1500,
          method: "direct_play",
          browser: "Brave",
          stopped_by_administrator: true,
        },
        WINDOWS_CHROME,
      ),
      t,
    );
    expect(said.title).toBe(
      "activity.watched(somebody|Lantern Street · home.up_next.short(2|5) · The Long Night)",
    );
    expect(said.note).toBe(
      "activity.played(work.minutes(25)) · method.direct_play · device.on(Brave|Windows) · activity.stopped_by_administrator",
    );
  });

  it("says which release started, without the commit it was built from", () => {
    expect(sayLine(line("server_started", { version: "0.1.0 (ade0738b)" }), t).title).toBe(
      "activity.server_started(0.1.0)",
    );
  });

  it("says a failed task with its reason, and a finished one with how long it took", () => {
    const failed = sayLine(
      line("task_failed", { task: "scan_library", on: { library: "Films" }, reason: "a folder could not be read" }),
      t,
    );
    expect(failed.title).toBe("activity.task_failed(jobs.scan_library · Films)");
    expect(failed.note).toBe("a folder could not be read");

    const finished = sayLine(line("task_finished", { task: "scan_library", on: null, took_seconds: 12 }), t);
    expect(finished.title).toBe("activity.task_finished(jobs.scan_library)");
    expect(finished.note).toBe("activity.took(activity.seconds(12))");
  });

  it("says who a sign in was, from where, and someone when the name is gone", () => {
    expect(sayLine(line("signed_in", { user_name: "somebody" }, WINDOWS_CHROME), t)).toEqual({
      title: "activity.signed_in(somebody)",
      note: "device.on(Chrome|Windows)",
    });
    expect(sayLine(line("signed_out", {}), t).title).toBe("activity.signed_out(activity.someone)");
  });

  it("says the name an account had and the one it has now", () => {
    expect(
      sayLine(line("account_renamed", { user_name: "somebody else", previous_name: "somebody" }), t).title,
    ).toBe("activity.account_renamed(somebody|somebody else)");
  });

  it("says what a deletion took and whether the disk lost it too", () => {
    const said = sayLine(
      line("works_deleted", { user_name: "somebody", titles: ["Quiet Harbour"], works: 1, from_the_disk: true }),
      t,
    );
    expect(said.title).toBe("activity.works_deleted(somebody|Quiet Harbour)");
    expect(said.note).toBe("activity.works_one · activity.from_the_disk");
  });
});

describe("whenItHappened", () => {
  const now = new Date(2026, 5, 10, 21, 0);

  it("gives the time alone today, yesterday with its word, and the day further back", () => {
    expect(whenItHappened(new Date(2026, 5, 10, 8, 5).toISOString(), now, "en", t)).toMatch(/08:05/);
    expect(whenItHappened(new Date(2026, 5, 9, 23, 30).toISOString(), now, "en", t)).toMatch(
      /^activity\.yesterday\(.*11:30/,
    );
    expect(whenItHappened(new Date(2026, 4, 2, 9, 0).toISOString(), now, "en", t)).toMatch(/May 2/);
    expect(whenItHappened(new Date(2025, 4, 2, 9, 0).toISOString(), now, "en", t)).toMatch(/2025/);
  });
});

describe("sayPoint", () => {
  it("leads each point to where it is put right", () => {
    expect(sayPoint({ point: "unidentified", state: "attention", may_be_seen: true, count: 3 }, t, "en")).toEqual({
      title: "attention.unidentified(3)",
      to: "/admin/metadata",
    });
    expect(
      sayPoint({ point: "refused_sign_ins", state: "attention", may_be_seen: true, count: 1 }, t, "en"),
    ).toEqual({ title: "attention.refused_sign_ins_one", to: "/admin/journal?families=access" });
    expect(
      sayPoint(
        { point: "worry", kind: "folder_missing", label: "disk-one", state: "trouble", may_be_seen: false },
        t,
        "en",
      ),
    ).toEqual({ title: "admin.worry.folder_missing(disk-one)", to: "/admin/libraries" });
  });
});
