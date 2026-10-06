//! Semantic highlighting for the transcript (Stage 13, `.debug/tagent-gui development
//! plan.md`).
//!
//! A transcript row's phrase/translation is rendered as `slint::StyledText` (a
//! CommonMark subset plus `<font color="...">`) instead of plain text, so different
//! parts of one wrapped paragraph can carry different colors. To keep that from
//! drifting apart from the plain-text formatting the popup and history logging still
//! rely on, colored blocks are built in two stages: a *template* -- markdown where a
//! colored span is `<font color="@role">...</font>`, the role name (never a literal
//! color) after `@` -- and a *render* of that template against the currently resolved
//! [`RoleColors`], redone whenever the palette changes (`main.rs`'s
//! `restyle_transcript`) without needing the original text again.
//!
//! Every user- or provider-derived string that ends up in a template must first go
//! through [`escape_markdown`] -- see that function's doc comment for exactly what it
//! guarantees, which is what keeps a role token embedded in hostile input from ever
//! reaching [`render_template_with_speaker`]'s substitution step.

use slint::{Color, StyledText, StyledTextFromMarkdownError};

/// Semantic role of one span of text in a template.
///
/// `Plain` and `Header` render in the block's own `default-color` (the existing
/// `phrase-color`/`translation-color` Slint properties) rather than a `<font>` span --
/// [`Role::token`] returns `None` for both.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    /// Ordinary text, no highlighting -- renders in the block's own default color.
    Plain,
    /// A dictionary article's header line (the primary translation) -- renders in the
    /// block's own default color, same as `Plain`; kept as a separate variant so
    /// call sites can say what a line *is* even though it isn't colored differently
    /// today.
    Header,
    /// The `[Language]:` prompt prefix on a phrase or translation line.
    Prompt,
    /// A dictionary entry's part-of-speech label line (e.g. "Noun").
    PartOfSpeech,
    /// The `[synonym, synonym]` bracket on a dictionary definition line.
    Synonym,
    /// A spelling-correction notice.
    Notice,
    /// An error message.
    Error,
}

impl Role {
    /// The `@`-token this role substitutes for inside a template's
    /// `<font color="@token">`, or `None` for the two roles that use the block's own
    /// default color instead of a `<font>` span at all.
    fn token(self) -> Option<&'static str> {
        match self {
            Role::Plain | Role::Header => None,
            Role::Prompt => Some("prompt"),
            Role::PartOfSpeech => Some("pos"),
            Role::Synonym => Some("synonym"),
            Role::Notice => Some("notice"),
            Role::Error => Some("error"),
        }
    }
}

/// Wraps already-[`escape_markdown`]-ed `text` in `role`'s `<font color="@role">` span,
/// or returns it unchanged for a role with no color of its own (`Plain`/`Header`).
///
/// `text` must already be escaped -- this never escapes on its own, so a caller that
/// forgets to escape first would let raw markdown/HTML through, or (worse) let a
/// literal `color="@pos"` substring reach [`render_template_with_speaker`]'s naive substitution.
pub fn span(role: Role, escaped_text: &str) -> String {
    match role.token() {
        Some(token) => format!("<font color=\"@{token}\">{escaped_text}</font>"),
        None => escaped_text.to_string(),
    }
}

/// A blank line inside a template (see [`escape_markdown`]).
///
/// A raw `\n\n` parses as an empty CommonMark paragraph, which `StyledText` collapses
/// to zero height (Stage 13 plan, gate G2) -- an NBSP-only line renders as one
/// full-height blank line instead, matching the gap a literal blank line gives the
/// plain-text formatters.
pub const BLANK_LINE: &str = "\u{a0}";

/// Escapes `text` for safe embedding in a template.
///
/// Every user- or provider-derived string (phrases, translations, dictionary
/// words/synonyms, error text, a `[label]:` prompt's label) must go
/// through this before being placed in a template, whether or not it's then wrapped in
/// a [`span`]. Guarantees, line by line (after normalizing `\r\n`/`\r` to `\n`):
///
/// - a line that is empty, or all whitespace, becomes a single [`BLANK_LINE`] (NBSP)
///   character -- see [`BLANK_LINE`]'s own doc comment for why;
/// - leading spaces/tabs become NBSP (Markdown strips leading spaces, and four or more
///   start an indented code block -- a hard parse error);
/// - trailing spaces become NBSP (two trailing spaces are a Markdown hard line break);
/// - every ASCII punctuation character in what's left is backslash-escaped -- valid
///   CommonMark, and this is what keeps a literal `color="@pos"` (or a bare `<`, `#`,
///   backtick, ...) in user text from ever being interpreted as markup or as a
///   [`render_template_with_speaker`] substitution target.
///
/// Escaping is **not** idempotent -- escaping already-escaped text double-escapes it
/// (every backslash this function inserts is itself ASCII punctuation, and gets a
/// second backslash on a second pass). Callers must escape raw text exactly once.
///
/// # Examples
///
/// ```
/// # // Doctest lives here for illustration; the crate's own tests exercise this
/// # // directly since `tagent-gui` is a binary crate.
/// ```
pub fn escape_markdown(text: &str) -> String {
    if text.is_empty() {
        // An empty *input* isn't a blank line to preserve spacing for -- it's simply
        // no text at all (e.g. an unused template field). Only a blank line embedded
        // within non-empty text becomes a `BLANK_LINE` marker, per the per-line rule
        // below.
        return String::new();
    }
    let normalized = text.replace("\r\n", "\n").replace('\r', "\n");
    normalized
        .split('\n')
        .map(escape_line)
        .collect::<Vec<_>>()
        .join("\n")
}

/// Builds a template that shows `text` line by line in a monospace font, for column-
/// aligned replies such as `/help`'s: each line becomes a Markdown code span, which
/// Slint draws in the generic monospace family (Consolas on Windows) whatever the
/// block's own font is. Unlike [`escape_markdown`] it never escapes, since a code
/// span shows backslashes literally, so `text` must be trusted: no backticks, and no
/// blank lines or lines with surrounding spaces (which a code span would trim).
pub fn code_lines(text: &str) -> String {
    text.lines()
        .map(|line| {
            debug_assert!(
                !line.contains('`') && !line.trim().is_empty() && line.trim() == line,
                "not code-span safe: {line:?}"
            );
            format!("`{line}`")
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn escape_line(line: &str) -> String {
    if line.trim().is_empty() {
        return BLANK_LINE.to_string();
    }
    let after_leading = line.trim_start_matches([' ', '\t']);
    let leading_count = line.len() - after_leading.len();
    let core = after_leading.trim_end_matches([' ', '\t']);
    let trailing_count = after_leading.len() - core.len();

    let nbsp = BLANK_LINE.chars().next().unwrap();
    let mut result = String::with_capacity(line.len() * 2);
    result.extend(std::iter::repeat_n(nbsp, leading_count));
    for ch in core.chars() {
        if ch.is_ascii_punctuation() {
            result.push('\\');
        }
        result.push(ch);
    }
    result.extend(std::iter::repeat_n(nbsp, trailing_count));
    result
}

/// Placeholder for the transcript's speaker glyph inside a `[label]:` prompt, before
/// its `]` -- see [`render_template_with_speaker`], which turns it into
/// [`SPEAKER_SUFFIX`] or removes it. Templates keep the marker rather than the glyph so
/// the glyph follows the live `enable_text_to_speech` setting on a re-render. User text
/// can never contain it: [`escape_markdown`] backslash-escapes both `@`s.
pub const SPEAKER_MARKER: &str = "@speaker@";

/// [`SPEAKER_MARKER`]'s counterpart for a block without a prompt: it starts the block
/// and turns into [`SPEAKER_PREFIX`].
pub const SPEAKER_LEAD_MARKER: &str = "@speaker-lead@";

/// What [`SPEAKER_MARKER`] renders to while text-to-speech is on: a non-breaking space
/// and the glyph, so a narrow window never wraps `[English 🔊]:` apart. `app.slint`
/// builds the same string to measure the prompt's clickable width -- keep both in step.
pub const SPEAKER_SUFFIX: &str = "\u{a0}🔊";

/// What [`SPEAKER_LEAD_MARKER`] renders to while text-to-speech is on: the glyph and a
/// non-breaking space (`🔊 text`).
pub const SPEAKER_PREFIX: &str = "🔊\u{a0}";

/// Best-effort plain-text fallback for a template that failed to render (see
/// [`render_template_with_speaker`]) -- strips `<...>` tags and un-escapes backslash-escaped
/// characters. Never fails; an ugly row beats a missing one.
pub fn strip_template(template: &str) -> String {
    let mut out = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '<' {
            for c2 in chars.by_ref() {
                if c2 == '>' {
                    break;
                }
            }
            continue;
        }
        if c == '\\' {
            if let Some(next) = chars.peek().copied() {
                out.push(next);
                chars.next();
                continue;
            }
        }
        out.push(c);
    }
    out
}

/// Per-role hex colors for one block -- see [`RoleColors::new`]. `Plain`/`Header`
/// aren't included: they render in the block's own `default-color`, which is a
/// separate Slint property, not one of these.
///
/// `prompt` is `String`, not `&'static str` like the other four: since the prompt
/// color became user-configurable (`prompt_color`/`popup_prompt_color` in
/// `tagent-gui.json`), it's a resolved runtime value, not one of two fixed presets --
/// see [`RoleColors::new`]. This is also why the struct as a whole is `Clone` but not
/// `Copy` any more.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleColors {
    /// Color for [`Role::Prompt`] -- resolved by the caller (theme-default-or-custom,
    /// same pattern as `phrase_color`/`translation_color`), not derived from `bg`.
    pub prompt: String,
    /// Color for [`Role::PartOfSpeech`].
    pub pos: &'static str,
    /// Color for [`Role::Synonym`].
    pub synonym: &'static str,
    /// Color for [`Role::Notice`].
    pub notice: &'static str,
    /// Color for [`Role::Error`].
    pub error: &'static str,
}

/// The four *automatically* derived role colors (everything but `prompt`) for one
/// block-background lightness class -- see [`RoleColors::new`].
struct AutoColors {
    pos: &'static str,
    synonym: &'static str,
    notice: &'static str,
    error: &'static str,
}

/// Picked for a light block background (Stage 13 plan, decision 5).
const LIGHT_AUTO_COLORS: AutoColors = AutoColors {
    pos: "#0b5cad",
    synonym: "#047857",
    notice: "#a16207",
    error: "#b91c1c",
};

/// Picked for a dark block background (Stage 13 plan, decision 5).
const DARK_AUTO_COLORS: AutoColors = AutoColors {
    pos: "#61afef",
    synonym: "#98c379",
    notice: "#d19a66",
    error: "#f87171",
};

/// The theme-default `prompt_color`/`popup_prompt_color` value under a light color
/// scheme -- mirrors `app.slint`'s `prompt-accent-theme-default`; there's no
/// Rust<->`.slint` constant sharing, so this is kept in sync by hand. Used as the
/// [`RoleColors::default`] filler and by tests; production code reads the resolved
/// value back from the window instead (`main.rs`'s `apply_style`/`apply_popup_style`).
pub const LIGHT_THEME_DEFAULT_PROMPT: &str = "#92400e";
/// The theme-default prompt color under a dark color scheme.
///
/// No production call site of its own (unlike [`LIGHT_THEME_DEFAULT_PROMPT`], the
/// `RoleColors::default` filler) -- kept as a real constant rather than an
/// inline test literal for symmetry with it and because both mirror the same pair
/// of literal values in `app.slint`.
#[allow(dead_code)]
pub const DARK_THEME_DEFAULT_PROMPT: &str = "#e5c07b";

impl Default for RoleColors {
    /// The light-background automatic set, with the light theme-default prompt.
    /// Only meaningful as a filler for a template that never colors a role at all
    /// (e.g. [`crate::info_transcript_entry`]'s plain-only rows) -- for anything
    /// that might actually render a colored span, use [`RoleColors::new`] with the
    /// caller's real resolved prompt color instead.
    fn default() -> Self {
        RoleColors::new(
            Color::from_rgb_u8(255, 255, 255),
            LIGHT_THEME_DEFAULT_PROMPT,
        )
    }
}

impl RoleColors {
    /// Builds the role colors for one block: `pos`/`synonym`/`notice`/`error` are
    /// still picked automatically from the relative luminance of `bg` -- the block's
    /// own *resolved* `phrase-background`/`translation-background` (custom hex, or
    /// the panel background it falls back to), not the raw OS theme, so they stay
    /// legible even when the user customized a block's background under a theme it
    /// doesn't match. `prompt` is **not** derived from `bg` -- unlike the other four
    /// roles, it's independently user-configurable (`prompt_color`/
    /// `popup_prompt_color`), so the caller passes in its already-resolved value
    /// (theme-default-or-custom, same pattern as `phrase_color`/`translation_color`).
    pub fn new(bg: Color, prompt: impl Into<String>) -> RoleColors {
        let auto = if is_dark(bg) {
            &DARK_AUTO_COLORS
        } else {
            &LIGHT_AUTO_COLORS
        };
        RoleColors {
            prompt: prompt.into(),
            pos: auto.pos,
            synonym: auto.synonym,
            notice: auto.notice,
            error: auto.error,
        }
    }
}

fn channel_luminance(c: u8) -> f64 {
    let c = f64::from(c) / 255.0;
    if c <= 0.03928 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

/// WCAG relative luminance of an sRGB color (0.0 = black, 1.0 = white).
fn relative_luminance(r: u8, g: u8, b: u8) -> f64 {
    0.2126 * channel_luminance(r) + 0.7152 * channel_luminance(g) + 0.0722 * channel_luminance(b)
}

fn is_dark(bg: Color) -> bool {
    relative_luminance(bg.red(), bg.green(), bg.blue()) < 0.5
}

/// Substitutes every role's `color="@token"` placeholder in `template` for its actual
/// hex color from `colors`. A literal `@token` string in user text can never match
/// this substring (it's always backslash-escaped by [`escape_markdown`] first, which
/// breaks up the exact `color="@token"` sequence), so this can never rewrite user
/// text -- see [`escape_markdown`]'s doc comment.
fn substitute_roles(template: &str, colors: &RoleColors) -> String {
    template
        .replace("color=\"@prompt\"", &format!("color=\"{}\"", colors.prompt))
        .replace("color=\"@pos\"", &format!("color=\"{}\"", colors.pos))
        .replace(
            "color=\"@synonym\"",
            &format!("color=\"{}\"", colors.synonym),
        )
        .replace("color=\"@notice\"", &format!("color=\"{}\"", colors.notice))
        .replace("color=\"@error\"", &format!("color=\"{}\"", colors.error))
}

/// Renders `template` against `colors`, exposing a parse failure instead of silently
/// falling back -- used by tests to assert that a template built from hostile input
/// never takes [`render_template_with_speaker`]'s fallback path.
#[cfg(test)]
pub fn render_template_checked(
    template: &str,
    colors: &RoleColors,
) -> Result<StyledText, StyledTextFromMarkdownError> {
    render_checked(template, colors, false)
}

fn render_checked(
    template: &str,
    colors: &RoleColors,
    speaker: bool,
) -> Result<StyledText, StyledTextFromMarkdownError> {
    StyledText::from_markdown(&substitute_roles(
        &substitute_speaker(template, speaker),
        colors,
    ))
}

/// Replaces [`SPEAKER_MARKER`] with [`SPEAKER_SUFFIX`] and [`SPEAKER_LEAD_MARKER`]
/// with [`SPEAKER_PREFIX`] (`speaker`), or both with nothing.
pub fn substitute_speaker(template: &str, speaker: bool) -> String {
    let (suffix, prefix) = if speaker {
        (SPEAKER_SUFFIX, SPEAKER_PREFIX)
    } else {
        ("", "")
    };
    template
        .replace(SPEAKER_MARKER, suffix)
        .replace(SPEAKER_LEAD_MARKER, prefix)
}

/// Renders `template` against `colors` into a `styled-text` value ready to bind to a
/// `StyledText` element; a [`SPEAKER_MARKER`] becomes the speaker glyph while `speaker`
/// (text-to-speech enabled) is on, and disappears otherwise. Never fails: a parse error
/// (which should never happen for a template this module built itself, but a row must
/// never simply vanish) logs once to stderr and falls back to [`strip_template`]'s
/// plain-text rendering.
pub fn render_template_with_speaker(
    template: &str,
    colors: &RoleColors,
    speaker: bool,
) -> StyledText {
    match render_checked(template, colors, speaker) {
        Ok(styled) => styled,
        Err(err) => {
            eprintln!(
                "Warning: failed to render styled transcript text ({err}); falling back to plain text"
            );
            StyledText::from_plain_text(&strip_template(&substitute_speaker(template, speaker)))
        }
    }
}

/// Builds an escaped `[label]:` prompt span (role [`Role::Prompt`]) followed by `body`,
/// or just `body` when `show_prompt` is off -- mirrors `format_line`'s plain-text
/// shape exactly (`"[{label}]: {text}"`). The label is the language pair before a
/// phrase (`auto → ru`) and the provider before a translation (`deepl`). With `speaker` (a transcript block that can
/// be spoken), the prompt gets a [`SPEAKER_MARKER`] before its `]` (`[label 🔊]:`), or,
/// with the prompt off, the block starts with a [`SPEAKER_LEAD_MARKER`] (`🔊 text`).
fn prefixed(show_prompt: bool, label: &str, body: &str, speaker: bool) -> String {
    if show_prompt {
        let marker = if speaker { SPEAKER_MARKER } else { "" };
        let prefix = format!(
            "{}{marker}{}",
            escape_markdown(&format!("[{label}")),
            escape_markdown("]:")
        );
        format!("{} {body}", span(Role::Prompt, &prefix))
    } else {
        let marker = if speaker { SPEAKER_LEAD_MARKER } else { "" };
        format!("{marker}{body}")
    }
}

/// Builds a phrase block's template: `text`, escaped, in the block's own default
/// color, behind an optional [`Role::Prompt`]-highlighted `[label]:` prefix (with a
/// [`SPEAKER_MARKER`] when `speaker`, see [`prefixed`]).
pub fn phrase_template(show_prompt: bool, label: &str, text: &str, speaker: bool) -> String {
    prefixed(show_prompt, label, &escape_markdown(text), speaker)
}

/// One transcript row's rendered Stage 13 fields: the two templates (kept so they can
/// be [`render_template_with_speaker`]-ed again after a restyle), the two rendered `styled-text`
/// values, and the two plain-text strings the right-click "Copy" menu items read.
///
/// Built by [`entry_fields`] so every `TranscriptEntry { .. }` construction site in
/// `main.rs` fills in the same six fields the same way, rather than repeating this
/// logic (and risking forgetting one) at each of the three call sites.
pub struct EntryFields {
    /// See [`Role`]/module docs -- the phrase block's template.
    pub phrase_template: String,
    /// The translation block's template.
    pub translation_template: String,
    /// The phrase block's rendered value, bound to `StyledText.text`.
    pub phrase_styled: StyledText,
    /// The translation block's rendered value.
    pub translation_styled: StyledText,
    /// Plain text the phrase block's right-click "Copy" menu item copies.
    pub phrase_copy: String,
    /// Plain text the translation block's right-click "Copy" menu item copies.
    pub translation_copy: String,
}

/// Renders `phrase_template`/`translation_template` against their own
/// `phrase_colors`/`translation_colors` -- separate, since decision 5 derives each
/// block's role colors from *that block's own* resolved background, which the user
/// can customize independently for the phrase and translation sides -- and packages
/// the result with `phrase_copy`/`translation_copy` into the six fields every
/// `TranscriptEntry { .. }` site needs. See [`EntryFields`]. `speaker` is whether
/// text-to-speech is on (see [`render_template_with_speaker`]).
pub fn entry_fields(
    phrase_template: String,
    translation_template: String,
    phrase_copy: String,
    translation_copy: String,
    phrase_colors: &RoleColors,
    translation_colors: &RoleColors,
    speaker: bool,
) -> EntryFields {
    let phrase_styled = render_template_with_speaker(&phrase_template, phrase_colors, speaker);
    let translation_styled =
        render_template_with_speaker(&translation_template, translation_colors, speaker);
    EntryFields {
        phrase_template,
        translation_template,
        phrase_styled,
        translation_styled,
        phrase_copy,
        translation_copy,
    }
}

/// Builds a translation block's template from an already-built `body` template (e.g.
/// a plain translation's [`escape_markdown`]-ed text, or a dictionary article's
/// multi-line template from [`crate::dictionary::to_template`]) -- adds the optional
/// `[label]:` prompt prefix, or (for `is_error`) wraps the whole thing in the `Error`
/// role and skips the prefix, mirroring `spawn_translation`'s plain-text handling
/// (an error message is never itself prompt-formatted). Unlike [`translation_template`],
/// `body` is used as-is -- the caller is responsible for having already escaped/
/// templated it, since it may already contain its own role-tagged spans (a dictionary
/// article's part-of-speech/synonym highlighting). `speaker` as in [`phrase_template`];
/// an error row never gets a marker.
pub fn translation_template_from_body(
    show_prompt: bool,
    label: &str,
    body: &str,
    is_error: bool,
    speaker: bool,
) -> String {
    if is_error {
        span(Role::Error, body)
    } else {
        prefixed(show_prompt, label, body, speaker)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- code_lines --------------------------------------------------------

    #[test]
    fn code_lines_wraps_each_line_in_a_code_span() {
        assert_eq!(code_lines("/l    swap\n/p    list"), "`/l    swap`\n`/p    list`");
    }

    /// The column padding survives Slint's Markdown parser: runs of spaces inside a
    /// code span are kept, and each line stays a line of its own.
    #[test]
    fn code_lines_keep_their_spacing_once_rendered() {
        let styled =
            StyledText::from_markdown(&code_lines("/l, /lang    swap\n/p <n>       use")).unwrap();
        let debug = format!("{styled:?}");
        assert!(debug.contains(r#"text: "/l, /lang    swap""#), "{debug}");
        assert!(debug.contains(r#"text: "/p <n>       use""#), "{debug}");
        assert_eq!(debug.matches("style: Code").count(), 2, "{debug}");
    }

    // --- escape_markdown ---------------------------------------------------

    #[test]
    fn escape_markdown_escapes_every_ascii_punctuation_char() {
        for byte in 0u8..=127 {
            let ch = byte as char;
            if !ch.is_ascii_punctuation() {
                continue;
            }
            let input = format!("a{ch}b");
            let expected = format!("a\\{ch}b");
            assert_eq!(
                escape_markdown(&input),
                expected,
                "punctuation char {ch:?} not escaped"
            );
        }
    }

    #[test]
    fn escape_markdown_converts_leading_whitespace_to_nbsp() {
        assert_eq!(escape_markdown("  hi"), "\u{a0}\u{a0}hi");
        assert_eq!(escape_markdown("\thi"), "\u{a0}hi");
    }

    #[test]
    fn escape_markdown_converts_trailing_whitespace_to_nbsp() {
        assert_eq!(escape_markdown("hi  "), "hi\u{a0}\u{a0}");
        assert_eq!(escape_markdown("hi\t"), "hi\u{a0}");
    }

    #[test]
    fn escape_markdown_normalizes_crlf_and_cr() {
        assert_eq!(escape_markdown("a\r\nb"), "a\nb");
        assert_eq!(escape_markdown("a\rb"), "a\nb");
    }

    #[test]
    fn escape_markdown_empty_string_is_empty() {
        assert_eq!(escape_markdown(""), "");
    }

    #[test]
    fn escape_markdown_leaves_non_ascii_untouched() {
        assert_eq!(escape_markdown("héllo мир 世界 🎉"), "héllo мир 世界 🎉");
    }

    #[test]
    fn escape_markdown_turns_empty_and_whitespace_only_lines_into_blank_line() {
        assert_eq!(escape_markdown("a\n\nb"), format!("a\n{BLANK_LINE}\nb"));
        assert_eq!(escape_markdown("a\n   \nb"), format!("a\n{BLANK_LINE}\nb"));
    }

    #[test]
    fn escape_markdown_is_not_idempotent() {
        let once = escape_markdown("a*b");
        let twice = escape_markdown(&once);
        assert_ne!(
            once, twice,
            "escaping twice must double-escape, not be a no-op"
        );
        assert_eq!(once, "a\\*b");
        assert_eq!(twice, "a\\\\\\*b");
    }

    // --- hostile input round-trips through render_template_with_speaker -----------------

    fn hostile_strings() -> Vec<&'static str> {
        vec![
            "# heading",
            "> quote",
            "a < b",
            "`code`",
            "1. item",
            "&amp;",
            "color=\"@pos\"",
            "<font color=\"@pos\">x</font>",
            "<b>bold</b>",
            "    four spaces",
            "**bold** _em_ ~~strike~~",
            "[link](http://example.com)",
        ]
    }

    #[test]
    fn hostile_input_never_takes_the_render_fallback_path() {
        let colors = RoleColors::new(
            Color::from_rgb_u8(255, 255, 255),
            LIGHT_THEME_DEFAULT_PROMPT,
        );
        for input in hostile_strings() {
            for role in [
                Role::Plain,
                Role::Header,
                Role::Prompt,
                Role::PartOfSpeech,
                Role::Synonym,
                Role::Notice,
                Role::Error,
            ] {
                let template = span(role, &escape_markdown(input));
                let result = render_template_checked(&template, &colors);
                assert!(
                    result.is_ok(),
                    "hostile input {input:?} under role {role:?} failed to parse: {:?}",
                    result.err()
                );
            }
        }
    }

    /// Mirrors `escape_line`'s leading/trailing whitespace -> NBSP transform (decision
    /// 4), without the punctuation escaping -- the expected *content* of a hostile
    /// string once it round-trips through escaping and markdown parsing back to
    /// literal text. Interior whitespace is untouched, matching `escape_line`.
    fn literal_transform(line: &str) -> String {
        if line.trim().is_empty() {
            return BLANK_LINE.to_string();
        }
        let after_leading = line.trim_start_matches([' ', '\t']);
        let leading_count = line.len() - after_leading.len();
        let core = after_leading.trim_end_matches([' ', '\t']);
        let trailing_count = after_leading.len() - core.len();
        format!(
            "{}{core}{}",
            BLANK_LINE.repeat(leading_count),
            BLANK_LINE.repeat(trailing_count)
        )
    }

    #[test]
    fn hostile_input_renders_as_literal_text() {
        let colors = RoleColors::new(
            Color::from_rgb_u8(255, 255, 255),
            LIGHT_THEME_DEFAULT_PROMPT,
        );
        for input in hostile_strings() {
            let template = phrase_template(false, "English", input, false);
            let rendered = render_template_with_speaker(&template, &colors, false);
            let expected = StyledText::from_plain_text(&literal_transform(input));
            assert_eq!(
                rendered, expected,
                "hostile input {input:?} did not render as literal text"
            );
        }
    }

    // --- RoleColors::new -----------------------------------------------------

    #[test]
    fn new_picks_dark_auto_set_for_black_background_and_keeps_given_prompt() {
        let colors = RoleColors::new(Color::from_rgb_u8(0, 0, 0), "#123456");
        assert_eq!(colors.pos, DARK_AUTO_COLORS.pos);
        assert_eq!(colors.synonym, DARK_AUTO_COLORS.synonym);
        assert_eq!(colors.notice, DARK_AUTO_COLORS.notice);
        assert_eq!(colors.error, DARK_AUTO_COLORS.error);
        assert_eq!(colors.prompt, "#123456");
    }

    #[test]
    fn new_picks_light_auto_set_for_white_background_and_keeps_given_prompt() {
        let colors = RoleColors::new(Color::from_rgb_u8(255, 255, 255), "#abcdef");
        assert_eq!(colors.pos, LIGHT_AUTO_COLORS.pos);
        assert_eq!(colors.synonym, LIGHT_AUTO_COLORS.synonym);
        assert_eq!(colors.notice, LIGHT_AUTO_COLORS.notice);
        assert_eq!(colors.error, LIGHT_AUTO_COLORS.error);
        assert_eq!(colors.prompt, "#abcdef");
    }

    fn contrast_ratio(fg: &str, bg: &str) -> f64 {
        let (fr, fg_, fb) = crate::parse_hex_color(fg).expect("valid hex");
        let (br, bgc, bb) = crate::parse_hex_color(bg).expect("valid hex");
        let l1 = relative_luminance(fr, fg_, fb);
        let l2 = relative_luminance(br, bgc, bb);
        let (lighter, darker) = if l1 > l2 { (l1, l2) } else { (l2, l1) };
        (lighter + 0.05) / (darker + 0.05)
    }

    /// `pos`/`synonym`/`notice`/`error` stay tied to the block's own background
    /// luminance (unlike `prompt`, now user-configurable -- see the next test).
    #[test]
    fn auto_role_colors_meet_wcag_contrast_against_reference_backgrounds() {
        for (colors, backgrounds) in [
            (&LIGHT_AUTO_COLORS, ["#ffffff", "#f5f5f5"]),
            (&DARK_AUTO_COLORS, ["#1e1e1e", "#282c34"]),
        ] {
            for bg in backgrounds {
                for (name, fg) in [
                    ("pos", colors.pos),
                    ("synonym", colors.synonym),
                    ("notice", colors.notice),
                    ("error", colors.error),
                ] {
                    let ratio = contrast_ratio(fg, bg);
                    assert!(
                        ratio >= 4.5,
                        "{name} ({fg}) against {bg} has contrast {ratio:.2}, need >= 4.5"
                    );
                }
            }
        }
    }

    /// The *theme-default* prompt colors (used when `prompt_color`/
    /// `popup_prompt_color` is left empty) against their own matching theme's
    /// reference backgrounds -- prompt is no longer paired with a block's
    /// background lightness the way the automatic roles above are, so this checks
    /// contrast against the theme it actually follows instead.
    #[test]
    fn theme_default_prompt_meets_wcag_contrast_against_its_own_theme_backgrounds() {
        for (fg, backgrounds) in [
            (LIGHT_THEME_DEFAULT_PROMPT, ["#ffffff", "#f5f5f5"]),
            (DARK_THEME_DEFAULT_PROMPT, ["#1e1e1e", "#282c34"]),
        ] {
            for bg in backgrounds {
                let ratio = contrast_ratio(fg, bg);
                assert!(
                    ratio >= 4.5,
                    "prompt ({fg}) against {bg} has contrast {ratio:.2}, need >= 4.5"
                );
            }
        }
    }

    // --- template shape ------------------------------------------------------

    #[test]
    fn phrase_template_without_prompt_has_no_prefix() {
        let template = phrase_template(false, "English", "hello", false);
        assert_eq!(template, "hello");
    }

    #[test]
    fn phrase_template_with_prompt_has_prompt_role_prefix() {
        let template = phrase_template(true, "English", "hello", false);
        assert_eq!(
            template,
            format!("{} hello", span(Role::Prompt, "\\[English\\]\\:"))
        );
    }

    // --- speaker marker ------------------------------------------------------

    #[test]
    fn speaker_marker_goes_inside_the_prompt() {
        let template = phrase_template(true, "English", "hello", true);
        let colors = RoleColors::default();
        assert!(render_checked(&template, &colors, true).is_ok());
        assert_eq!(
            strip_template(&substitute_speaker(&template, true)),
            "[English\u{a0}🔊]: hello"
        );
        assert_eq!(
            strip_template(&substitute_speaker(&template, false)),
            "[English]: hello"
        );
    }

    #[test]
    fn speaker_marker_starts_the_block_without_a_prompt() {
        let template = phrase_template(false, "English", "hello", true);
        assert_eq!(
            strip_template(&substitute_speaker(&template, true)),
            "🔊\u{a0}hello"
        );
        assert_eq!(
            strip_template(&substitute_speaker(&template, false)),
            "hello"
        );
    }

    #[test]
    fn no_speaker_marker_unless_asked_or_on_error_rows() {
        assert!(!phrase_template(true, "English", "hello", false).contains(SPEAKER_MARKER));
        let error = translation_template_from_body(
            true,
            "Russian",
            &escape_markdown("Error: boom"),
            true,
            true,
        );
        assert!(!error.contains(SPEAKER_MARKER));
        assert!(!error.contains(SPEAKER_LEAD_MARKER));
    }

    #[test]
    fn speaker_marker_in_user_text_is_never_substituted() {
        let template = phrase_template(false, "English", "a @speaker@ @speaker-lead@ b", false);
        assert!(!template.contains(SPEAKER_MARKER), "{template}");
        assert!(!template.contains(SPEAKER_LEAD_MARKER), "{template}");
        assert_eq!(
            strip_template(&substitute_speaker(&template, true)),
            "a @speaker@ @speaker-lead@ b"
        );
    }

    #[test]
    fn translation_template_from_body_error_row_is_whole_text_in_error_role() {
        let template = translation_template_from_body(
            true,
            "Russian",
            &escape_markdown("Error: boom"),
            true,
            false,
        );
        assert_eq!(template, span(Role::Error, "Error\\: boom"));
    }

    #[test]
    fn translation_template_from_body_non_error_respects_show_prompt() {
        let body = escape_markdown("привет");
        let with_prompt = translation_template_from_body(true, "Russian", &body, false, false);
        assert_eq!(
            with_prompt,
            format!("{} привет", span(Role::Prompt, "\\[Russian\\]\\:"))
        );
        let without_prompt = translation_template_from_body(false, "Russian", &body, false, false);
        assert_eq!(without_prompt, "привет");
    }
}
