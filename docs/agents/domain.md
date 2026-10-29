# Domain Docs

This repo uses a single-context layout: `GLOSSARY.md` at the root and architecture decision records in `docs/adr/`.

## Before exploring, read these

- Read `GLOSSARY.md` at the repo root for domain terminology.
- Read ADRs in `docs/adr/` that touch the area you are about to work in.

If these files do not exist, proceed silently. Do not flag their absence or suggest creating them upfront. The `/domain-modeling` skill creates them lazily when terms or decisions actually get resolved.

## Use the glossary's vocabulary

When your output names a domain concept (in an issue title, a refactor proposal, a hypothesis, or a test name), use the term defined in `GLOSSARY.md` rather than synonyms the glossary explicitly avoids.

If a concept is missing, reconsider whether the project needs it; note genuine gaps for `/domain-modeling`.

## Flag ADR conflicts

If a proposal contradicts an existing ADR, identify the ADR and explain why the decision may need reopening instead of silently overriding it.
