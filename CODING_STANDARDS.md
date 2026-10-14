# Coding standards

The rules a reviewer checks by reading a diff. Each one takes judgement, so
no tool enforces it. Mechanical rules are left to `scripts/check.ps1`:
rustfmt, clippy (including cognitive complexity ≤ 10 and no `MutexGuard` held across
`.await`), `dotnet format`, and no `TODO`/`FIXME` in source. Don't re-check
those by hand.

## Sources to read alongside this file

- **`GLOSSARY.md`**: the domain vocabulary. Names in code, tests, log
  messages and commit messages use its terms, not synonyms it rules out.
- **`docs/adr/`**: accepted decisions. A change must not contradict one. If
  it has to, the diff includes a new or amended ADR.
- **`src-tauri/src/db/migrations/README.md`**: the migration rules. Any diff
  that touches `src-tauri/src/db/migrations/` follows them.
- **`docs/ux/tone.md`**: the tone of voice (constitution §III). Any diff that
  adds or changes user-facing text in `src/Tracey.App` follows it.

## Structure

1. **One responsibility per function.** If describing what a function does
   takes "and", it should probably be two. A function over ~40 logical lines
   is a prompt to look, not a violation.
2. **Every `#[allow(...)]` carries its reason** in a comment on the same or
   the preceding line. An allow for `clippy::cognitive_complexity` says why
   the function is better left whole.
3. **No commented-out code.** Delete it; git has it.
4. **No new `#[allow(dead_code)]`.** The lint is how unused code gets found;
   delete the code instead of silencing it.

## Data and privacy

5. **Collect nothing new without saying so.** A diff that records a new kind
   of user data (a new column, a new captured field) links the issue that
   decided it.
6. **Logs carry no personal data, at any level.** Window titles, process
   names tied to a time, OCR text, screenshot paths, time-entry descriptions,
   connection strings and credentials never appear in a log message. IDs and
   error text are fine.
7. **Log levels mean what they say.** `debug` for diagnostic detail, `info`
   for lifecycle events, `warn` for something recovered from, `error` for a
   failure that needs attention.
8. **New defaults favour privacy.** A new preference or setting defaults to
   collecting less and sending nothing off the device.
9. **Nothing leaves the device without opt-in.** Code that sends data to a
   new destination only runs behind a setting the user turned on for that
   destination.
10. **Tests use synthetic data.** No real window titles, screenshots or
    activity recordings in fixtures.

## Security

11. **SQL takes values as parameters.** Values go through `?N` / `$N`
    placeholders, never through `format!`. Formatting is fine only for
    placeholder lists and fixed identifiers chosen in code.
12. **No hard-coded secrets or keys.** Credentials come from the keychain
    (`keyring`) or the user.
13. **Validate input at the boundary.** Paths, URIs and other values arriving
    through a Tauri command are checked before use. A path from the user can't
    escape the directory it's meant for.
14. **No unauthenticated local endpoints.** A new local socket, HTTP
    listener or IPC channel other than Tauri commands checks who is calling.
15. **New dependencies are pinned.** A version in `Cargo.toml` or a
    `.csproj` is exact or minimum-compatible (`"1.2"`, `Version="1.2.0"`): no
    `*` or floating version, no git branch. One whose purpose involves network calls or telemetry gets a
    comment beside it saying why that's acceptable.

## Errors and the UI

16. **Errors the user sees say what happened, why it matters to them, and
    what they can do.** No raw error strings, stack traces or internal IDs in
    UI text.
17. **No new silently dropped errors.** A `Result` discarded with
    `let _ =`, `.ok()` or `.unwrap_or_default()` is logged at `warn` or above,
    or has a comment saying why it can be ignored.

## Tests

18. **Behaviour changes come with tests.** A diff that changes behaviour adds
    or updates a test that would fail without it.
19. **Tests are deterministic.** No real sleeps, wall-clock time or ordering
    luck. Inject the clock, the sleep or the source instead.
20. **Test names describe the behaviour** in glossary terms, not the function
    under test.
