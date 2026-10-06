//! The Settings dialog's "About" tab: its links, built from `Cargo.toml`'s `homepage`
//! (the user book) and `repository`, so the URLs have one source.
//!
//! The links render in a `StyledText`, whose `link-clicked` opens the URL in the
//! default browser: through `open-link` (on Linux `session::open_url`, which needs to
//! give the browser back `WAYLAND_DISPLAY`), else Slint's `Platform.open-url`.

use slint::StyledText;

/// The user book (`docs/user`, published to GitHub Pages).
pub const BOOK_URL: &str = env!("CARGO_PKG_HOMEPAGE");

/// The source repository.
pub const REPOSITORY_URL: &str = env!("CARGO_PKG_REPOSITORY");

/// The About tab's links as Markdown, one per line.
pub fn links_markdown() -> String {
    let repository = REPOSITORY_URL.trim_end_matches('/');
    [
        format!("User guide: [{}]({BOOK_URL})", display_url(BOOK_URL)),
        format!("Source code: [{}]({repository})", display_url(repository)),
        format!(
            "Report a problem: [{}/issues]({repository}/issues)",
            display_url(repository)
        ),
    ]
    .join("\n")
}

/// [`links_markdown`], parsed for the tab's `StyledText`.
pub fn links() -> StyledText {
    StyledText::from_markdown(&links_markdown()).expect("the About links are valid Markdown")
}

/// `url` as shown: without the scheme and a trailing slash.
fn display_url(url: &str) -> &str {
    url.trim_start_matches("https://").trim_end_matches('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_come_from_cargo_toml() {
        assert_eq!(BOOK_URL, "https://holgertkey.github.io/tagent/");
        assert_eq!(REPOSITORY_URL, "https://github.com/holgertkey/tagent");
    }

    #[test]
    fn links_name_the_book_the_repository_and_its_issues() {
        let markdown = links_markdown();
        assert!(markdown
            .contains("[holgertkey.github.io/tagent](https://holgertkey.github.io/tagent/)"));
        assert!(markdown
            .contains("[github.com/holgertkey/tagent](https://github.com/holgertkey/tagent)"));
        assert!(markdown.contains(
            "[github.com/holgertkey/tagent/issues](https://github.com/holgertkey/tagent/issues)"
        ));
        assert_eq!(markdown.lines().count(), 3);
    }

    #[test]
    fn links_parse() {
        StyledText::from_markdown(&links_markdown()).unwrap();
    }
}
