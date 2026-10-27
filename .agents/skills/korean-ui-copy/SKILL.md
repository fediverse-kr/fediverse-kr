---
name: korean-ui-copy
description: Use when editing Korean UI copy. Preserve meaning.
version: 0.1.0
license: MIT
---

# Korean UI copy

Project-local adaptation of epoko77-ai/im-not-ai for fediverse.kr. Use it when
editing Korean introductory copy, explanations, and help text, not code or stored
member content. The upstream MIT source is pinned as the `references/im-not-ai`
git submodule; its commit is the provenance, not a floating downloaded rulebook.

## Procedure

1. Read `references/im-not-ai/skills/humanize-korean/references/quick-rules.md`.
   Initialize the reference with `git submodule update --init references/im-not-ai`
   if needed. Do not run the upstream global installer or install its Git hooks.
2. Record the exact original and proposed strings. Keep names, addresses, numbers,
   technical terms, conditions, claim strength, and the existing level of politeness.
3. Apply only rules that fit UI text: unnecessary particles and commas, literal
   translation phrasing, redundant modifiers, and accidentally unfinished prose.
   Keep intentional short labels, headings, examples, and fictional dialogue.
4. Do not apply essay sentence-length targets or forced stylistic variation to
   buttons, cards, captions, or lists. Do not replace “할 수 있어요” with a guarantee.
5. Compare each pair for meaning and check the actual affected screens. For an
   aggregate over-editing check, save the before/after strings outside the source
   tree and run `python3 references/im-not-ai/scripts/verify_change_rate.py
   --before <before-file> --after <after-file>`; investigate warnings rather than
   padding the input to dilute the rate.

## Limits and verification

This is a conservative UI adaptation, not the upstream multi-agent light/standard/
heavy pipeline. Report exactly which checks ran. A low change rate does not prove
naturalness or meaning preservation, and an essay score is not a UI acceptance gate.
Use direct comparison, the real build, and browser inspection. Keep content changes
separate from functionality changes; never infer that applying this skill authorizes
new features, global configuration changes, or an additional agent review cycle.

The submodule is an authoring reference only; it must not enter application assets,
Cargo dependencies, or the runtime container. Preserve its upstream license.
