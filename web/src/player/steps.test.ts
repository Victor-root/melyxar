/*
 * What a picker of lengths shows for the length a viewer has.
 */

import { describe, expect, it } from "vitest";
import { offeredAs, TYPED_IN } from "./steps";

describe("a length as a picker shows it", () => {
  it("is the length itself when it is one of those offered", () => {
    expect(offeredAs(10, false)).toBe("10");
    expect(offeredAs(30, true)).toBe("30");
  });

  it("is typed in by hand for anything else", () => {
    expect(offeredAs(12, false)).toBe(TYPED_IN);
    expect(offeredAs(90, true)).toBe(TYPED_IN);
  });

  it("is none only where none is offered", () => {
    expect(offeredAs(0, true)).toBe("0");
    expect(offeredAs(0, false)).toBe(TYPED_IN);
  });
});
