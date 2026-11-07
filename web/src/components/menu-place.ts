/*
 * Where a menu drawn over the page stands, once it is known how large it came
 * out and how much room the window leaves.
 */

/** How close a menu comes to the edge of the window, and to what it is beside. */
const OFF_THE_EDGE = 8;
const FROM_THE_BUTTON = 6;

type Box = Pick<DOMRect, "top" | "right" | "bottom" | "left">;

interface Size {
  width: number;
  height: number;
}

/** Above the button when there is no room under it, and pulled back from the
 *  edge rather than hanging off it. */
function underButton(size: Size, button: Box, window: Size) {
  const under = button.bottom + FROM_THE_BUTTON;
  return {
    top:
      under + size.height > window.height - OFF_THE_EDGE
        ? Math.max(OFF_THE_EDGE, button.top - size.height - FROM_THE_BUTTON)
        : under,
    left: Math.min(
      Math.max(OFF_THE_EDGE, button.right - size.width),
      window.width - size.width - OFF_THE_EDGE,
    ),
  };
}

/**
 * Where a menu stands: beside what it is about when that is given and the
 * window has the room, on its right first, so that it covers none of it; under
 * its button otherwise.
 */
export function menuPlace(size: Size, button: Box, beside: Box | undefined, window: Size) {
  if (!beside) {
    return underButton(size, button, window);
  }
  const onTheRight = beside.right + OFF_THE_EDGE;
  const onTheLeft = beside.left - OFF_THE_EDGE - size.width;
  const left =
    onTheRight + size.width <= window.width - OFF_THE_EDGE
      ? onTheRight
      : onTheLeft >= OFF_THE_EDGE
        ? onTheLeft
        : null;
  if (left === null) {
    return underButton(size, button, window);
  }
  return {
    top: Math.max(OFF_THE_EDGE, Math.min(beside.top, window.height - size.height - OFF_THE_EDGE)),
    left,
  };
}
