import { describe, expect, it } from "vitest";
import { creationOptions, fromBase64Url, isADomainName, requestOptions, toBase64Url } from "./passkeys";

describe("passkeys", () => {
  it("writes bytes as unpadded base64 for the web, and reads them back", () => {
    const bytes = new Uint8Array([251, 255, 0, 62, 63, 1]);
    const written = toBase64Url(bytes);
    expect(written).toBe("-_8APj8B");
    expect([...fromBase64Url(written)]).toEqual([...bytes]);
    expect([...fromBase64Url("AQ")]).toEqual([1]);
    expect([...fromBase64Url("AQ==")]).toEqual([1]);
  });

  it("tells a domain name from an address", () => {
    expect(isADomainName("melyxar.example.org")).toBe(true);
    expect(isADomainName("192.168.1.21")).toBe(false);
    expect(isADomainName("[fe80::1]")).toBe(false);
    expect(isADomainName("")).toBe(false);
  });

  it("turns the written pieces of the options into bytes, and leaves the rest", () => {
    const made = creationOptions({
      publicKey: {
        challenge: "AQID",
        rp: { id: "melyxar.example.org", name: "Melyxar" },
        user: { id: "BAU", name: "victor", displayName: "victor" },
        excludeCredentials: [{ type: "public-key", id: "Bg" }],
      },
    });
    expect([...new Uint8Array(made.challenge as ArrayBuffer)]).toEqual([1, 2, 3]);
    expect([...new Uint8Array(made.user.id as ArrayBuffer)]).toEqual([4, 5]);
    expect(made.user.name).toBe("victor");
    expect(made.rp.id).toBe("melyxar.example.org");
    expect([...new Uint8Array(made.excludeCredentials?.[0].id as ArrayBuffer)]).toEqual([6]);

    const asked = requestOptions({ publicKey: { challenge: "Bw", rpId: "melyxar.example.org" } });
    expect([...new Uint8Array(asked.challenge as ArrayBuffer)]).toEqual([7]);
    expect(asked.allowCredentials).toBeUndefined();
    expect(asked.rpId).toBe("melyxar.example.org");
  });
});
