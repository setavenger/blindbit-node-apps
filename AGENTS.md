# AGENTS.md — blindbit-node-apps

Packaging for running BlindBit's `friglet` daemon on node-in-a-box platforms
(Umbrel, StartOS, RaspiBlitz, myNode, and others surveyed in `upstream/others/`).
One repository for every platform. Do not split a platform into its own repo.

## Layout rules

- `upstream/<platform>/` holds material pulled **verbatim** from each platform provider
  (docs, templates, reference apps). Never edit those files. Every pulled file set is
  listed in that folder's `SOURCES.md` with URL, upstream commit or retrieval date, and
  license. Our own distilled notes live next to them in `NOTES.md` (or `SURVEY.md`).
- To refresh upstream material, re-pull it and update `SOURCES.md` in the same commit.
- Our packaging code lives outside `upstream/`. Nothing in the packaging may import or
  copy from `upstream/` at build time; it is reference material only.

## Non-negotiables

- friglet source lives in `blindbit-rs`. This repo consumes it by pinned commit or
  image digest; it never vendors or patches friglet source. Fixes to friglet go to
  `blindbit-rs` as their own pull requests.
- GitHub: pushing a branch and `gh pr create` are the only write actions. Read-only `gh`
  otherwise. Merging is the maintainer's call.
- Branch flow: work lands on `integration` (or a feature branch merged into it).
  `master` only moves after the maintainer has reviewed.
- Use bare tracker IDs (for example `SNB-123`) in commits and docs, never tracker URLs.
- Prove changes work. Build the images and run the checks; quote the outcome. Say
  plainly what was not verified.
- Secrets: the friglet scan key is a secret. It must never appear in logs, images,
  compose files, or committed fixtures.
