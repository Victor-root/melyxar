import { describe, expect, it } from "vitest";
import type { AccessStatus } from "../../api";
import { addressAfter, changed, draftOf, namesOf, runningOut } from "./access";

describe("addressAfter", () => {
  it("sends a page in the clear to https once the server encrypts", () => {
    expect(
      addressAfter(
        "self_signed",
        true,
        new URL("http://192.168.1.10:2100/admin/security?x=1"),
      ),
    ).toBe("https://192.168.1.10:2100/admin/security?x=1");
  });

  it("leaves a page already encrypted where it is", () => {
    expect(
      addressAfter(
        "provided",
        true,
        new URL("https://media.example/admin/security"),
      ),
    ).toBeNull();
  });

  it("brings an encrypted page back to the clear behind a proxy", () => {
    expect(
      addressAfter(
        "proxy",
        true,
        new URL("https://192.168.1.10:2100/admin/security"),
      ),
    ).toBe("http://192.168.1.10:2100/admin/security");
  });

  it("leaves a page the proxy encrypts alone", () => {
    expect(
      addressAfter(
        "proxy",
        false,
        new URL("https://media.example/admin/security"),
      ),
    ).toBeNull();
  });
});

describe("runningOut", () => {
  const now = new Date("2026-09-30T12:00:00Z");

  it("says so within a month of the end, and after it", () => {
    expect(runningOut("2026-10-15T00:00:00Z", now)).toBe(true);
    expect(runningOut("2026-01-01T00:00:00Z", now)).toBe(true);
    expect(runningOut("2027-09-30T00:00:00Z", now)).toBe(false);
  });
});

describe("changed", () => {
  const status: AccessStatus = {
    mode: "self_signed",
    certificate_path: null,
    private_key_path: null,
    redirect_to_https: true,
    public_names: ["media.example.org"],
    certificate: null,
    problem: null,
  };

  it("has nothing to apply when nothing was touched", () => {
    expect(changed(status, draftOf(status))).toBe(false);
  });

  it("sees a mode, a redirection and a list of names that differ", () => {
    expect(changed(status, { ...draftOf(status), mode: "proxy" })).toBe(true);
    expect(changed(status, { ...draftOf(status), redirect: false })).toBe(true);
    expect(
      changed(status, {
        ...draftOf(status),
        names: "media.example.org, 203.0.113.7",
      }),
    ).toBe(true);
  });

  it("does not see a different way of writing the same names", () => {
    expect(
      changed(status, { ...draftOf(status), names: "  media.example.org ," }),
    ).toBe(false);
  });

  it("wants both files of a certificate that is given", () => {
    const given = { ...draftOf(status), mode: "provided" as const };
    expect(
      changed(
        {
          ...status,
          mode: "provided",
          certificate_path: "/a.pem",
          private_key_path: "/a.key",
        },
        { ...given, certificatePath: "/a.pem", keyPath: "/a.key" },
      ),
    ).toBe(false);
    expect(
      changed(
        {
          ...status,
          mode: "provided",
          certificate_path: "/a.pem",
          private_key_path: "/a.key",
        },
        { ...given, certificatePath: "/b.pem", keyPath: "/a.key" },
      ),
    ).toBe(true);
  });

  it("ignores what means nothing behind a proxy", () => {
    const behind = { ...status, mode: "proxy" as const };
    expect(
      changed(behind, {
        ...draftOf(behind),
        redirect: false,
        names: "other.example.org",
      }),
    ).toBe(false);
  });
});

describe("namesOf", () => {
  it("splits on commas and spaces", () => {
    expect(namesOf("a.org, b.org  c.org,")).toEqual([
      "a.org",
      "b.org",
      "c.org",
    ]);
  });
});
