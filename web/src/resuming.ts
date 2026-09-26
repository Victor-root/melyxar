/*
 * The rules a viewer holds a film left partway to, read and changed kind of
 * library by kind of library.
 */

import type { LibraryKind, ResumeRules } from "./api";

/** The rules each kind given some holds, as the server keeps them. */
export type RulesByKind = (ResumeRules & { kind: LibraryKind })[];

/** The rules of one kind: its own, or those of every kind until it has some. */
export function rulesOf(byKind: RulesByKind, kind: LibraryKind, everyKind: ResumeRules): ResumeRules {
  const own = byKind.find((given) => given.kind === kind);
  return own
    ? { min_percent: own.min_percent, max_percent: own.max_percent, min_seconds: own.min_seconds }
    : everyKind;
}

/** Every kind's rules, with one kind's put in place of what it had. */
export function withRulesOf(byKind: RulesByKind, kind: LibraryKind, rules: ResumeRules): RulesByKind {
  return [...byKind.filter((given) => given.kind !== kind), { kind, ...rules }];
}
