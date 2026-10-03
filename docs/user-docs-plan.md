# User Documentation Plan

Living document: the plan for Tagent's user documentation, a book for the people who use
`tagent-cli` and `tagent-gui`. Update it when a decision is made or a step lands; this
document *is* the current state of the plan.

Where the rest lives:
- **How things work inside** (for developers): [`ARCHITECTURE.md`](ARCHITECTURE.md), the
  project's `CLAUDE.md`, and the `tagent` rustdoc (docs.rs). These stay developer
  documentation; the book doesn't replace them.
- **What changed and when**: each crate's `CHANGELOG.md`.
- **Per-app roadmaps**: [`tagent-cli-dev-plan.md`](tagent-cli-dev-plan.md),
  [`tagent-gui-dev-plan.md`](tagent-gui-dev-plan.md),
  [`providers-dev-plan.md`](providers-dev-plan.md).

**Status:** in progress. The proposals of UD-Q1–Q6 were accepted on 2026-10-03; D0–D4 are
done (D3 still lacks the screenshots that need clicks).

---

## Problem

- **`tagent-cli/README.md` has grown to 771 lines.** It mixes everything in one file:
  - a feature overview;
  - installation;
  - a usage guide and the interactive commands;
  - a configuration reference;
  - provider profiles and API keys;
  - examples, troubleshooting, and building from source.

  It is too long to be a README and hard to use as a reference: there is no navigation
  and no search.
- **The hardest part is shared, but documented per app.** Provider profiles (DeepL, the
  OpenAI-compatible provider with Ollama, LM Studio and OpenRouter), the three
  independent axes (translation, dictionary, speech), `TAGENT_<PROFILE>_<KEY>`
  variables, and prompts and `response_format` work the same in both apps. Today:
  - they are explained in `tagent-cli/README.md` (in TOML);
  - `tagent-gui/README.md` (177 lines) touches only some of them.

  Each new provider grows this duplication.
- **`tagent-gui/README.md` lags between releases by design.** It is updated only on a
  semver bump, so it describes the last version, not what the app can do now.
- **No task-oriented material.** Nothing answers "how do I translate with a local model",
  "why is the dictionary silent", or "the hotkey doesn't work on Wayland" as a page of its
  own. Today the answers are scattered across READMEs and changelog entries.

## Goal

One user book, written in Markdown in this repository and published as a website:
- **Organization:** by the user's tasks, not by crate. It covers both applications and
  shares the provider part between them.
- **Staying in sync:** it grows with the code, and cheap checks keep its references in
  sync with the code.
- **The READMEs:** they shrink to an introduction, a quick start, and a link to the book.

## Audience

- **In scope:** people who use `tagent-cli` or `tagent-gui`, from a first install to
  custom provider profiles.
- **Out of scope:** developers building on the `tagent` library. Their documentation is
  the rustdoc on docs.rs plus `tagent/README.md`, both unchanged by this plan, and
  contributors have `ARCHITECTURE.md` and the dev plans.

## Current material (what the book starts from)

| Source | Lines | Goes to |
|--------|------:|---------|
| `README.md` (workspace) | 48 | stays; gains a link to the book |
| `tagent-cli/README.md` | 771 | most of it moves into the book; the README keeps an introduction |
| `tagent-gui/README.md` | 177 | "What it does" and "Configuration" move into the book |
| `tagent/README.md` | 113 | stays as is (library users, crates.io) |
| Changelog entries | — | source for recipes and troubleshooting (e.g. the `{}` answer from small models) |
| `tagent-cli.toml` template (`--print-default-config`) | — | the configuration reference |
| `tagent`'s provider registry (`OptionSpec`s) | — | the provider options reference |

## Questions to settle

Each was settled by its proposal on 2026-10-03; the alternatives are kept for the record.

- **UD-Q1. Tool?**
  - **Proposed:** [mdBook](https://rust-lang.github.io/mdBook/) (0.5.x).
    - It is plain Markdown plus `SUMMARY.md`, with navigation, search, light and dark
      themes, and `{{#include}}` for generated snippets.
    - It is a Rust tool, the same toolchain as the project.
    - In CI, install it as the mdBook docs recommend for CI:
      `cargo install mdbook --no-default-features --features search --vers "^0.5" --locked`.
  - **Alternative:** plain `.md` files in `docs/user/`, rendered by GitHub. No build and no
    site, but also no search, no side navigation and no includes. This is a fallback if
    publishing is not wanted.
- **UD-Q2. When does the site update?**
  - **Proposed:** on a `v*` tag push, as part of the release, so the site describes the
    latest release, the version users download. Without a tag the site doesn't change.
  - **Alternative:** every push to `main`. The site would then describe unreleased
    behavior.
- **UD-Q3. When is the book edited?**
  - **Proposed:** together with the change it describes, in the same commit, the way a
    changelog entry is written today. With UD-Q2's release-only publishing, an early edit
    shows nothing before the release. A "book" item joins the stage checklists next to
    "changelog".
    - Documentation-only edits don't bump `+BUILD` (existing rule).
  - **Alternative:** only at a semver bump, like `tagent-gui/README.md` today. Cheaper
    per change, but the release then needs a catch-up pass, and it gets skipped.
  - **Consequence of the proposal:** the `tagent-gui` README cadence rule becomes almost
    moot. The shortened README rarely changes.
- **UD-Q4. What stays in the READMEs?** The crates.io page of each app is its README.
  - **Proposed:** a short README per app:
    - what it is;
    - a feature list with one line each;
    - install (download, `cargo install`, `.deb`);
    - a 5-line quick start;
    - links to the book, the changelog and the license.

    `tagent-cli/README.md` keeps the three lines `build.rs` syncs: the title
    `# Tagent Text Translator v...`, `**Current Version**: v...`, and the footer. If they
    are dropped, `build.rs` and `release.yml`'s "docs in sync" check must change with them.
  - **Alternative:** keep the READMEs complete and duplicate the material. Rejected,
    because duplication is the problem this plan solves.
- **UD-Q5. Language?**
  - **Proposed:** English only, as the project's rule for all text requires.
  - A Russian translation is out of scope. mdBook has no built-in multi-language support,
    so a translation would be a second book.
- **UD-Q6. Where does it live?**
  - **Proposed:** `docs/user/` in this repository: `book.toml`, with `src/` for the pages
    and `src/images/` for screenshots. The build output `docs/user/book/` goes into
    `.gitignore`.
  - This keeps a feature and its documentation in one commit (UD-Q3).
  - A separate repository or a `gh-pages` branch would be the alternative; neither is
    needed with Pages deployed from Actions.

## Proposed structure (`SUMMARY.md`)

```
Introduction                      what Tagent is, CLI vs GUI: which one to pick
Getting started
  Install                         downloads, .deb, cargo install, Linux packages
  First translation (tagent-cli)
  First translation (tagent-gui)
tagent-cli
  Modes                           unified (hotkeys + prompt), CLI one-shot
  Hotkeys                         formats, double-press, speech hotkey, restart rule
  Interactive commands            /l, /p (all forms), /s, /ss, /save, /config ...
  Dictionary and spell check
  Text-to-speech
  History
  Colors
tagent-gui
  The main window                 transcript, input, copy, provider menu
  Hotkeys and the popup
  Tray and startup                start_minimized, hide to tray, desktop entry
  Settings                        tab by tab
Providers                         shared by both apps
  How providers work              three axes, profiles, built-in vs profile names,
                                  session choices vs saved defaults
  Google                          the default, no setup
  DeepL                           key, free vs pro endpoint, limits
  OpenAI-compatible               endpoint/model/api_key, prompts, response_format,
                                  temperature, timeouts
    Recipe: Ollama (local)
    Recipe: LM Studio (local)
    Recipe: OpenAI
    Recipe: OpenRouter
    Recipe: one profile for translation and the dictionary
  API keys and environment variables   TAGENT_<PROFILE>_<KEY>, where keys are stored
Reference
  tagent-cli.toml                 every section and key (generated, see below)
  tagent-gui.json                 every key
  Provider options                every option of every kind (generated)
  Command-line options            --help, --config, --update-config, ...
  Supported languages             generated from tagent::languages::LANGUAGES
  File locations                  config, history, logs per OS
Troubleshooting
  Platforms                       Wayland without X11, macOS stubs, hotkey conflicts
  Providers                       missing options, 401/403/429, quota, slow local
                                  models, "the dictionary is silent"
  Upgrading                       --update-config, the startup notice
```

## Keeping it in sync (the checks)

A book drifts unless the parts that mirror the code are checked. Cheap checks only:

1. **The book builds in CI.** `mdbook build docs/user` runs in `ci.yml` (Linux job). It
   catches a broken `SUMMARY.md` and a missing `{{#include}}` file.
2. **Generated reference pages**, written by a test or a small script and committed:
   - **`tagent-cli.toml`:** the template, written with a fixed target language. Today's
     `--print-default-config` depends on the machine's locale, so the generator needs
     `render_config` with `target_language = "en"`.
   - **Provider options:** a table per kind, from `tagent`'s registry (key, required,
     secret, default, description).
   - **Supported languages:** from `tagent::languages::LANGUAGES`.

   A test fails when a committed page differs from what the generator produces now, and
   says how to regenerate it. This is the pattern `tagent-gui`'s `.desktop` test already
   uses. The tests live in the app crates, not in `tagent`: a `tagent` test reading
   `../docs` would break `cargo package` verification.
3. **Every interactive command is documented.** A `tagent-cli` test checks that each
   entry of `SLASH_COMMANDS` appears on the "Interactive commands" page. The same check
   for `--help` options against "Command-line options".
4. **Link check:** proposed as out of scope for now. `mdbook-linkcheck` is a third-party
   preprocessor and an extra CI dependency; revisit if broken links show up.

## Steps

Each step is one commit, unless noted otherwise:
- **Version bumps:** documentation-only steps bump no version. A step that adds a test or
  generator code to an app is a code change, so it bumps that app's `+BUILD` with a
  changelog entry.
- **Third-party tools:** read the official documentation of mdBook and of the Pages
  actions at the start of the step that uses them, as the project rule requires.

### D0 — Skeleton and CI build

- `docs/user/book.toml`, `src/SUMMARY.md` with the structure above, every page a stub.
- `.gitignore`: `docs/user/book/`.
- `ci.yml`: install mdBook (cached), run `mdbook build docs/user`.
- **Done when:** CI builds the empty book; `mdbook serve docs/user` shows the navigation
  locally.
- **Done** (2026-10-03):
  - Every page exists as a one-line stub, so D1–D5 only fill pages.
  - `create-missing = false`: a page listed in `SUMMARY.md` but missing on disk fails the
    build instead of being created silently.
  - CI caches `~/.cargo/bin/mdbook` on its own (the cargo cache doesn't cover
    `~/.cargo/bin`).
  - `mdbook serve` needs mdBook's default features; the CI install leaves them out.

### D1 — Providers (first, highest value)

- "How providers work", Google, DeepL, OpenAI-compatible, the recipes, and "API keys and
  environment variables". Written once for both apps, each example shown twice, as a TOML
  block for `tagent-cli` and the Settings path for `tagent-gui`.
- **Recipes are verified, not recalled:**
  - Ollama was live-tested during P2/P3.
  - The others (LM Studio, OpenAI, OpenRouter) take their endpoint URLs and model names
    from each service's official documentation, read during this step and linked from the
    recipe.
  - A recipe that hasn't been run says so.
- **Done when:** a user can set up DeepL or a local Ollama from the book alone, in both
  apps.
- **Done** (2026-10-03):
  - The Ollama recipe was run as written against a local Ollama (`tagent-cli`, with
    `qwen2.5:3b`: a phrase and a dictionary lookup).
  - LM Studio, OpenAI and OpenRouter take their URLs from each service's documentation
    (linked) and say they are untested.
  - The OpenAI recipe names `gpt-5.4-mini` from OpenAI's model list, with a note that
    model names change.

### D2 — tagent-cli pages

- Move "Usage Guide", "Interactive Commands", "Configuration", "Customizing Hotkeys",
  "Examples", "Advanced Usage" and the CLI part of "Troubleshooting" from
  `tagent-cli/README.md` into the book, edited for the new structure (no copy-paste
  duplicates).
- The README isn't shortened yet (D6), so for one step the material exists twice.
- **Done** (2026-10-03):
  - The `cli/` pages, "First translation (tagent-cli)", the hand-written part of the
    `tagent-cli.toml` reference (location, syntax, reload, errors, how the app writes
    it), "Command-line options", "Upgrading", and the `tagent-cli` part of
    "Troubleshooting: Platforms". D4 adds the generated template listing to the
    `tagent-cli.toml` page; D5 adds the rest of troubleshooting.
  - The `/p` and `/l` examples are real output (prompt-only run without `DISPLAY`).
  - Found while writing, fixed in D4 (`tagent-cli` 0.17.0+019): the template's
    `history_file` comment and two `--help` lines.

### D3 — tagent-gui pages

- Main window, the hotkey popup, tray, and Settings tab by tab, with screenshots.
- **Screenshots:**
  - Theme: light theme, the default window size.
  - Storage: kept in `src/images/` and compressed (PNG, kept small).
  - Capture method: launch with `--foreground`, show the window through the tray's
    D-Bus `Activate`, then `import -window "Tagent"` under X11. This is what this project
    already uses; don't drive the GUI with `xdotool` keypresses. A screenshot that needs
    a click (an open menu) is taken by hand.
- **Also in D3** (no step had them): "Introduction", "Install" (both apps: downloads,
  `cargo install` with the Linux build packages, the `.deb`, building from source) and
  "First translation (tagent-gui)". Install and building from source exist only in the
  READMEs today, and D6 shortens them.
- **Done when:** every Settings tab and the provider menu have a page section.
- **Done** (2026-10-03):
  - The `gui/` pages, "Introduction", "Install" and "First translation (tagent-gui)".
    Written from `app.slint` and the code, not from `tagent-gui/README.md`, which lags
    (it still lists five languages and the providers on the General tab).
  - Screenshot `images/gui-main-window.png` (600×567): a non-interactive launch with an
    isolated `XDG_CONFIG_HOME` (`theme: light`, `start_minimized: false`, hotkeys off),
    `WAYLAND_DISPLAY` unset for the X11 backend, `import -window`. Its transcript is
    empty, since filling it needs input.
  - **Open, by hand:** a main window with a phrase and a dictionary entry (to replace the
    empty one), the popup (`gui-popup.png`, for "Hotkeys and the popup"), Settings >
    General and Settings > Providers (`gui-settings-general.png`,
    `gui-settings-providers.png`, for "Settings"), and the provider menu open. The pages
    have no image references for them yet.

### D4 — Reference pages and their checks

- The generators and tests of "Keeping it in sync", points 2 and 3, plus the
  `tagent-gui.json` and file-location pages.
- **Version bumps:** `tagent-cli` and `tagent-gui` get a `+BUILD` each, since this step
  adds test code.
- **Done** (2026-10-03; `tagent-cli` 0.17.0+019, `tagent-gui` 0.15.0+005):
  - `tagent-cli/src/user_docs.rs` (test-only) generates `reference/generated/`
    `tagent-cli.toml` (target `en`, history path `<data folder>/...`),
    `provider-options.md` (per kind: its jobs, options with required/secret/default from
    the descriptor's `TransportDefaults`, the built-in prompts) and `languages.md`; the
    pages pull them in with `{{#include}}`. A stale file fails with the command to
    regenerate: `TAGENT_UPDATE_USER_DOCS=1 cargo test -p tagent-cli user_docs`.
  - Checks that every `SLASH_COMMANDS` entry and every `--help` flag (`HELP_OPTIONS`, the
    list `--help` now prints from) appears as code on its page; a `tagent-gui` test does
    the same for every key of a default `tagent-gui.json`.
  - The tests read `../docs/user` at run time and skip when it's absent (a crates.io
    copy), so packaging is unaffected. Each check was seen failing on a broken page.
  - Found and fixed in the same build: on macOS the config folder is
    `~/Library/Application Support` (`dirs` 5), not `~/.config` as the READMEs and code
    comments said; the template's `history_file` comment ("relative to the program
    directory"; it is the working directory); `--help`'s hotkey lines ("copied to
    clipboard automatically", "anywhere in Windows"). The prompt's input history is
    `interactive_history.txt` in the config folder.

### D5 — Troubleshooting

- Collected from the READMEs and from changelog entries that describe a user-visible
  failure:
  - Wayland;
  - `BadAccess` on a hotkey grab;
  - missing options;
  - 401/403/429 and quota errors;
  - a small model answering `{}`;
  - a broken custom prompt showing only as "no dictionary".
  - "Translation failed" in general: network, firewall, the service unavailable (from
    `tagent-cli/README.md`).

### D6 — Shorten the READMEs

- Per UD-Q4. The workspace `README.md` links to the book instead of to the long app
  README.
- **Checks:** `cargo build -p tagent-cli` (the `build.rs` markers) and the release
  workflow's "docs in sync" check still pass.

### D7 — Publish

- A `pages` job in `release.yml` (per UD-Q2): build the book, then
  `actions/configure-pages`, `actions/upload-pages-artifact`, `actions/deploy-pages`, with
  `pages: write` and `id-token: write` permissions. Read their current versions and inputs
  from the official documentation at the time.
- **Needs the user:**
  - one change in the repository settings (Settings > Pages > Source: "GitHub Actions");
  - the first deploy happens with the next release (or a manual `workflow_dispatch`
    run, if wanted).
- Set `homepage` (or `documentation`) in `tagent-cli/Cargo.toml` and
  `tagent-gui/Cargo.toml` to the book's URL, so crates.io links to it.

### D8 — Process

- `ARCHITECTURE.md`: a "User documentation" section (decided 2026-10-03: not `CLAUDE.md`,
  which is gitignored, so its rules wouldn't reach the repository). It covers:
  - where the book lives;
  - "a user-visible change edits the book in the same commit" (UD-Q3);
  - how to regenerate the reference pages;
  - "Adding a New Translation Provider" gains "add its page under Providers".
- The dev plans' stage templates gain a "Book" item next to "Changelog".
- `tagent-gui`'s README cadence note is revised accordingly (UD-Q3).

## Done when

- The book is published, and its URL is in both apps' READMEs and on crates.io.
- `tagent-cli/README.md` is short; nothing in it exists only there.
- CI fails when the book doesn't build, when a generated reference page is stale, or when
  an interactive command is undocumented.
- A new provider or command has an obvious place in the book and a rule that says it must
  be written.

## Out of scope

- A Russian (or any other) translation of the book (UD-Q5).
- Developer documentation: `ARCHITECTURE.md`, the dev plans and rustdoc stay where they
  are.
- Versioned docs (one site per release): the site always shows the latest release.
- In-app help beyond today's `/help` and `--help` (e.g. a "Help" button in `tagent-gui`
  that opens the book): a small follow-up once the URL exists.
- Automated link checking (see "Keeping it in sync", point 4).
