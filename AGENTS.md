# AGENTS.md

Repo-level configuration for agent skills working in this repository.

## Checks

Before committing, run `pwsh scripts/check.ps1`. It runs every check CI runs. When a skill says to run typechecking, linting or tests, this script is what it means.

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `VincentVerweij/tracey`, managed via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

The five canonical triage labels, used unchanged: `needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`. See `docs/agents/triage-labels.md`.

### Domain docs

Single-context layout — `GLOSSARY.md` plus `docs/adr/` at the repo root. See `docs/agents/domain.md`.
