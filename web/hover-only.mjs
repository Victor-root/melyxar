/*
 * A touch screen has no pointer, yet it reports a press as a hover that lasts
 * until the next press somewhere else: a button held a little long came up lit,
 * grown or ringed the way it does under a mouse. Every rule written for the
 * hover is therefore moved, in what the build gives out, into a media query
 * that only holds where something can hover, in the place of the rule it
 * came from so that nothing is ever overridden differently.
 */

/** A list of selectors cut at its commas, those inside brackets left alone. */
function cut(selector) {
  const parts = [];
  let depth = 0;
  let from = 0;
  for (let at = 0; at < selector.length; at += 1) {
    const char = selector[at];
    if (char === "(" || char === "[") {
      depth += 1;
    } else if (char === ")" || char === "]") {
      depth -= 1;
    } else if (char === "," && depth === 0) {
      parts.push(selector.slice(from, at).trim());
      from = at + 1;
    }
  }
  parts.push(selector.slice(from).trim());
  return parts.filter(Boolean);
}

/** Whether a selector is about the pointer being over something. */
const hovered = (selector) => /:hover/.test(selector.replace(/:not\([^)]*:hover[^)]*\)/g, ""));

/** Whether a rule is somewhere a hover has no business being moved from. */
function exempt(rule) {
  for (let parent = rule.parent; parent; parent = parent.parent) {
    if (parent.type !== "atrule") {
      continue;
    }
    if (/keyframes/.test(parent.name) || (parent.name === "media" && /hover|pointer/.test(parent.params))) {
      return true;
    }
  }
  return false;
}

export default function hoverOnlyWhereThereIsHover() {
  return {
    postcssPlugin: "hover-only-where-there-is-hover",
    Rule(rule, { AtRule }) {
      if (exempt(rule)) {
        return;
      }
      const parts = cut(rule.selector);
      const over = parts.filter(hovered);
      if (over.length === 0) {
        return;
      }
      const media = new AtRule({ name: "media", params: "(hover: hover)" });
      media.append(rule.clone({ selector: over.join(", ") }));
      rule.after(media);
      const kept = parts.filter((selector) => !hovered(selector));
      if (kept.length > 0) {
        rule.selector = kept.join(", ");
      } else {
        rule.remove();
      }
    },
  };
}

hoverOnlyWhereThereIsHover.postcss = true;
