import { describe, expect, it } from "vitest";
import { layoutOf, libraryViewOf, viewsRead, viewsWritten, withView } from "./library-views";

describe("the layouts of the libraries", () => {
  it("reads back what it wrote", () => {
    const views = { films: "list", series: "thumbnails" } as const;
    expect(viewsRead(viewsWritten(views))).toEqual(views);
  });

  it("leaves out what does not read as a layout", () => {
    expect(viewsRead("films:mosaic,series:list,:list,lonely,shows:posters")).toEqual({ series: "list" });
    expect(viewsRead(null)).toEqual({});
  });

  it("takes a library back to its posters by forgetting it", () => {
    const views = withView({ films: "list" }, "series", "compact_posters");
    expect(views).toEqual({ films: "list", series: "compact_posters" });
    expect(withView(views, "films", "posters")).toEqual({ series: "compact_posters" });
  });

  it("knows only the layouts there are", () => {
    expect(libraryViewOf("thumbnails")).toBe("thumbnails");
    expect(libraryViewOf("mosaic")).toBe("posters");
    expect(libraryViewOf(undefined)).toBe("posters");
  });

  it("keeps the shape a library has by nature where the layout does not set one", () => {
    expect(layoutOf("posters", "lying")).toEqual({ grid: "lying", card: "lying" });
    expect(layoutOf("list", "lying")).toEqual({ grid: "list", card: "lying" });
    expect(layoutOf("thumbnails", "standing")).toEqual({ grid: "lying", card: "lying" });
    expect(layoutOf("compact_posters", "lying")).toEqual({ grid: "compact", card: "standing" });
  });
});
