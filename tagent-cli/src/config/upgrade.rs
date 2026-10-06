//! Keeping an existing `tagent-cli.toml` in step with the template of the running version
//! (Stage C in `docs/tagent-cli-dev-plan.md`): warnings about keys this version doesn't
//! know, and adding the settings a file lacks. The template ([`config_template`]) is the
//! only source of known sections and keys, so the lists can't drift from what a new file
//! contains.
//!
//! The file is never rewritten on its own: [`upgrade`] runs only on an explicit command,
//! and only adds (keys, sections, comments), never removes or renames.

use super::{config_template, profile_example, provider_kinds, render_config, Config};
use std::collections::HashSet;
use toml_edit::{DocumentMut, Item, TableLike};

/// The section holding the provider profiles: free-form, checked per profile kind.
const PROFILES: &str = "provider_options";

/// A key or section in the config file that this version doesn't know.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct UnknownKey {
    /// 1-based line of the key (or section header) in the file, when known.
    pub line: Option<usize>,
    /// What is unknown, e.g. ``unknown key `x` in [interface]``.
    pub what: String,
    /// A likely intended name, e.g. ``did you mean `y`?``.
    pub hint: Option<String>,
}

impl UnknownKey {
    /// The warning for `file_name`, the hint (if any) on a second, indented line:
    ///
    /// ```text
    /// Warning: tagent-cli.toml line 12: unknown key `auto_hide_seconds` in [interface]
    ///          (did you mean `auto_hide_terminal_seconds`?)
    /// ```
    pub fn warning(&self, file_name: &str) -> String {
        let mut text = match self.line {
            Some(line) => format!("Warning: {file_name} line {line}: {}", self.what),
            None => format!("Warning: {file_name}: {}", self.what),
        };
        if let Some(hint) = &self.hint {
            text.push_str(&format!("\n         ({hint})"));
        }
        text
    }

    /// The same information on one line, without the file name (for the
    /// `--update-config` report).
    pub fn summary(&self) -> String {
        let mut text = match self.line {
            Some(line) => format!("line {line}: {}", self.what),
            None => self.what.clone(),
        };
        if let Some(hint) = &self.hint {
            text.push_str(&format!(" ({hint})"));
        }
        text
    }
}

/// The template's sections, each with its keys, in template order.
fn known_sections() -> Vec<(String, Vec<String>)> {
    let template: DocumentMut = config_template()
        .parse()
        .expect("the config template is valid TOML");
    template
        .iter()
        .filter_map(|(name, item)| {
            let keys = item.as_table_like()?.iter().map(|(k, _)| k.to_string());
            Some((name.to_string(), keys.collect()))
        })
        .collect()
}

/// The keys and sections of `doc` that this version doesn't know: keys and sections
/// missing from the template, and options a profile's provider kind doesn't declare in
/// `tagent`'s registry (a profile of an unknown kind isn't checked: building it already
/// reports that). `raw` is the text `doc` was parsed from, for line numbers.
///
/// Takes a [`toml_edit::Document`] rather than a [`DocumentMut`], since only the former
/// keeps the spans the line numbers come from.
pub(super) fn unknown_keys(doc: &toml_edit::Document<String>) -> Vec<UnknownKey> {
    let raw = doc.raw();
    let line_of = |key: Option<&toml_edit::Key>| {
        key.and_then(|k| k.span())
            .map(|span| raw[..span.start].matches('\n').count() + 1)
    };
    let known = known_sections();
    let mut unknown = Vec::new();
    for (name, item) in doc.iter() {
        let line = line_of(doc.key(name));
        if name == PROFILES {
            if let Some(profiles) = item.as_table_like() {
                unknown.extend(unknown_profile_options(profiles, &line_of));
            }
            continue;
        }
        let Some(table) = item.as_table_like() else {
            // A plain key outside any section: say where it belongs.
            unknown.push(UnknownKey {
                line,
                what: format!("unknown key `{name}` outside any section"),
                hint: key_hint(name, None, &known),
            });
            continue;
        };
        let Some((_, keys)) = known.iter().find(|(section, _)| section == name) else {
            let sections = known
                .iter()
                .map(|(section, _)| section.as_str())
                .chain([PROFILES]);
            unknown.push(UnknownKey {
                line,
                what: format!("unknown section [{name}]"),
                hint: suggest(name, sections).map(|s| format!("did you mean [{s}]?")),
            });
            continue;
        };
        for (key, _) in table.iter() {
            if !keys.iter().any(|k| k == key) {
                unknown.push(UnknownKey {
                    line: line_of(table.key(key)),
                    what: format!("unknown key `{key}` in [{name}]"),
                    hint: key_hint(key, Some(name), &known),
                });
            }
        }
    }
    unknown
}

/// Options of `[provider_options.<name>]` profiles that their provider kind (the `type`
/// option, else the profile name) doesn't declare.
fn unknown_profile_options(
    profiles: &dyn TableLike,
    line_of: &dyn Fn(Option<&toml_edit::Key>) -> Option<usize>,
) -> Vec<UnknownKey> {
    let kinds = provider_kinds();
    let mut unknown = Vec::new();
    for (profile, item) in profiles.iter() {
        let Some(options) = item.as_table_like() else {
            continue;
        };
        let kind = options
            .get("type")
            .and_then(Item::as_str)
            .unwrap_or(profile)
            .to_lowercase();
        let Some((_, descriptors)) = kinds.iter().find(|(name, _)| *name == kind) else {
            continue;
        };
        let mut declared = vec!["type"];
        for option in descriptors.iter().flat_map(|d| d.options) {
            if !declared.contains(&option.key) {
                declared.push(option.key);
            }
        }
        for (key, _) in options.iter() {
            // Profile keys are case-insensitive (`ProviderProfiles` lowercases them).
            if !declared.contains(&key.to_lowercase().as_str()) {
                unknown.push(UnknownKey {
                    line: line_of(options.key(key)),
                    what: format!("unknown option `{key}` in [{PROFILES}.{profile}]"),
                    hint: suggest(key, declared.iter().copied())
                        .map(|k| format!("did you mean `{k}`?")),
                });
            }
        }
    }
    unknown
}

/// A hint for an unknown `key` found in `section` (`None`: outside any section), looking
/// in every known section: the same name elsewhere means the key belongs there, and a
/// similar name is a likely typo. A candidate in `section` itself wins a tie.
fn key_hint(key: &str, section: Option<&str>, known: &[(String, Vec<String>)]) -> Option<String> {
    let mut best: Option<(usize, &str, &str)> = None;
    for (name, keys) in known {
        for candidate in keys {
            let Some(score) = similarity(key, candidate) else {
                continue;
            };
            let better = match best {
                None => true,
                Some((best_score, best_section, _)) => {
                    score < best_score
                        || (score == best_score
                            && Some(name.as_str()) == section
                            && Some(best_section) != section)
                }
            };
            if better {
                best = Some((score, name, candidate));
            }
        }
    }
    let (score, name, candidate) = best?;
    Some(if Some(name) == section {
        format!("did you mean `{candidate}`?")
    } else if score == 0 && candidate == key {
        format!("`{candidate}` belongs in [{name}]")
    } else {
        format!("did you mean `{candidate}` in [{name}]?")
    })
}

/// The candidate closest to `name` (see [`similarity`]), if any is close enough.
fn suggest<'a>(name: &str, candidates: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    candidates
        .filter_map(|candidate| similarity(name, candidate).map(|score| (score, candidate)))
        .min_by_key(|(score, _)| *score)
        .map(|(_, candidate)| candidate)
}

/// How far `name` is from `candidate` (lower is closer; `0` = equal ignoring case), or
/// `None` when they are too different for a suggestion. Close means a small edit
/// distance (a typo, `apikey` for `api_key`), or every `_`-separated word of `name`
/// appearing in `candidate` (a shortened name, `auto_hide_seconds` for
/// `auto_hide_terminal_seconds`), as long as those words aren't too short to mean much.
fn similarity(name: &str, candidate: &str) -> Option<usize> {
    let (name, candidate) = (name.to_lowercase(), candidate.to_lowercase());
    let distance = edit_distance(&name, &candidate);
    let longer = name.chars().count().max(candidate.chars().count());
    if distance <= (longer / 4).max(1) {
        return Some(distance);
    }
    let words: Vec<&str> = name.split('_').filter(|w| !w.is_empty()).collect();
    let candidate_words: Vec<&str> = candidate.split('_').collect();
    let letters: usize = words.iter().map(|w| w.len()).sum();
    (letters >= 6 && words.iter().all(|w| candidate_words.contains(w))).then_some(distance)
}

/// The Levenshtein distance between `a` and `b`, in characters.
fn edit_distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut previous: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut current = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let substitution = previous[j] + usize::from(ca != *cb);
            current.push(substitution.min(previous[j + 1] + 1).min(current[j] + 1));
        }
        previous = current;
    }
    previous[b.len()]
}

/// The line of the template that starts the explanation of provider profiles (followed
/// by the example profiles); a file that has it has them.
const PROFILES_MARKER: &str = "# Provider profiles";

/// The start of the line that introduces the example profiles, after the explanation
/// (since 0.17.0+004; older files have the explanation without them).
const EXAMPLES_MARKER: &str = "# Ready-made profiles";

/// What a new config file contains: the template with the default values.
fn reference() -> DocumentMut {
    render_config(&Config::default())
        .parse()
        .expect("the rendered config template is valid TOML")
}

/// The `(section, key)` pairs that `content` has as commented-out settings: comment lines
/// of the form `# key = ...` (any indentation and number of `#`), under the section header
/// above them. A commented-out header (`# [speech]`) starts a section as well, so a whole
/// section can be commented out.
fn commented_out_keys(content: &str) -> HashSet<(String, String)> {
    let header = |line: &str| {
        let name = line.strip_prefix('[')?.trim_start_matches('[');
        let end = name.find(']')?;
        Some(name[..end].trim().to_string())
    };
    let mut section = String::new();
    let mut keys = HashSet::new();
    for line in content.lines().map(str::trim) {
        let comment = line
            .strip_prefix('#')
            .map(|c| c.trim_start_matches('#').trim_start());
        match comment {
            None => {
                if let Some(name) = header(line) {
                    section = name;
                }
            }
            Some(comment) => {
                if let Some(name) = header(comment) {
                    section = name;
                    continue;
                }
                let key_end = comment
                    .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
                    .unwrap_or(comment.len());
                let (key, rest) = comment.split_at(key_end);
                if !key.is_empty() && rest.trim_start().starts_with('=') {
                    keys.insert((section.clone(), key.to_string()));
                }
            }
        }
    }
    keys
}

/// The template's settings missing from `doc` (parsed from `content`), as `(section, key)`
/// in template order. A key the file has commented out in its section counts as present:
/// that is how a user declines a setting for good.
fn missing_settings(
    content: &str,
    doc: &DocumentMut,
    reference: &DocumentMut,
) -> Vec<(String, String)> {
    let commented = commented_out_keys(content);
    let mut missing = Vec::new();
    for (section, item) in reference.iter() {
        let Some(template) = item.as_table_like() else {
            continue;
        };
        let present = doc.get(section).and_then(Item::as_table_like);
        for (key, _) in template.iter() {
            let found = present.is_some_and(|table| table.contains_key(key))
                || commented.contains(&(section.to_string(), key.to_string()));
            if !found {
                missing.push((section.to_string(), key.to_string()));
            }
        }
    }
    missing
}

/// How many of the template's settings `content` lacks (see [`missing_settings`]); `0`
/// for a file that isn't valid TOML.
pub(super) fn new_settings_count(content: &str) -> usize {
    match content.parse::<DocumentMut>() {
        Ok(doc) => missing_settings(content, &doc, &reference()).len(),
        Err(_) => 0,
    }
}

/// The result of [`upgrade`]: the new file content and what was added, one line each.
#[derive(Debug)]
pub(super) struct Upgrade {
    /// The whole file with the additions.
    pub content: String,
    /// What was added, e.g. `[speech] speech_provider`.
    pub added: Vec<String>,
}

/// `content` (a config file the application accepts) with what the template has and the
/// file lacks added: missing keys at the end of their section, with the template's
/// comments above them and their default values; missing sections whole, after the
/// section that precedes them in the template; and at the end, the provider profile
/// explanation (unless the file has its [`PROFILES_MARKER`] line) and the example
/// profiles (unless it has their [`EXAMPLES_MARKER`] line; if it has, the example block of
/// each provider kind it lacks, see [`has_example`]). Nothing is
/// removed, renamed or reordered, and a commented-out key is not added back. `None` when
/// nothing is missing.
pub(super) fn upgrade(content: &str) -> Result<Option<Upgrade>, toml_edit::TomlError> {
    let mut doc: DocumentMut = content.parse()?;
    let reference = reference();
    let missing = missing_settings(content, &doc, &reference);
    let mut added = Vec::new();
    // The file's first section, which gets a blank line above it when a missing section
    // goes before it.
    let first_section = doc
        .iter()
        .filter_map(|(name, item)| Some((item.as_table()?.position()?, name.to_string())))
        .min()
        .map(|(_, name)| name);
    // Where a missing section goes: after the last template section the file has, or
    // first of all (position 0 is the root's).
    let mut position = Some(0);
    let mut inserted_first = false;
    for (section, item) in reference.iter() {
        let Some(template) = item.as_table() else {
            continue;
        };
        let keys: Vec<&str> = missing
            .iter()
            .filter(|(s, _)| s == section)
            .map(|(_, k)| k.as_str())
            .collect();
        match doc.get_mut(section) {
            Some(existing) => {
                if let Some(pos) = existing.as_table().and_then(|t| t.position()) {
                    position = Some(pos);
                }
                // A regular `[section]` gets the keys with their comments; a dotted or
                // inline one just the plain `key = value`.
                let formatted = existing.as_table().is_some_and(|t| !t.is_dotted());
                let Some(table) = existing.as_table_like_mut() else {
                    continue;
                };
                for key in keys {
                    let (template_key, value) = template
                        .get_key_value(key)
                        .expect("missing keys come from the template");
                    if formatted {
                        let mut template_key = template_key.clone();
                        let decor = template_key.leaf_decor_mut();
                        let prefix = decor
                            .prefix()
                            .and_then(|p| p.as_str())
                            .unwrap_or("")
                            .to_string();
                        if !table.is_empty() && !prefix.starts_with('\n') {
                            decor.set_prefix(format!("\n{prefix}"));
                        }
                        table.entry_format(&template_key).or_insert(value.clone());
                    } else if let Some(value) = value.as_value() {
                        let mut value = value.clone();
                        value.decor_mut().clear();
                        table.insert(key, Item::Value(value));
                    }
                    added.push(format!("[{section}] {key}"));
                }
            }
            None if keys.is_empty() => {}
            None => {
                let mut table = template.clone();
                table.retain(|key, _| keys.contains(&key));
                // Only the comment right above the header (the template's first section
                // carries the file's introduction, which the file already has).
                let prefix = table
                    .decor()
                    .prefix()
                    .and_then(|p| p.as_str())
                    .unwrap_or("");
                let paragraph = prefix
                    .rsplit("\n\n")
                    .next()
                    .unwrap_or("")
                    .trim_start_matches('\n')
                    .to_string();
                // No blank line above the very first thing in the file.
                let goes_first = position == Some(0) && !inserted_first;
                let separator = if goes_first && doc.iter().all(|(_, i)| i.is_table()) {
                    ""
                } else {
                    "\n"
                };
                inserted_first |= position == Some(0);
                table
                    .decor_mut()
                    .set_prefix(format!("{separator}{paragraph}"));
                table.set_position(position);
                doc.insert(section, Item::Table(table));
                added.push(if keys.len() == template.len() {
                    let plural = if keys.len() == 1 { "" } else { "s" };
                    format!("[{section}] (section, {} setting{plural})", keys.len())
                } else {
                    format!("[{section}] {}", keys.join(", "))
                });
            }
        }
    }
    if let (true, Some(first)) = (inserted_first, first_section) {
        if let Some(table) = doc.get_mut(&first).and_then(Item::as_table_mut) {
            let prefix = table
                .decor()
                .prefix()
                .and_then(|p| p.as_str())
                .unwrap_or("")
                .to_string();
            if !prefix.starts_with('\n') {
                table.decor_mut().set_prefix(format!("\n{prefix}"));
            }
        }
    }
    // The profiles explanation and the examples at the end of the template. A file from
    // before the examples existed (0.17.0+003 and older) has the explanation only: it
    // gets just the examples, so the explanation isn't there twice.
    let has_line = |marker: &str| content.lines().any(|line| line.starts_with(marker));
    let explanation = reference.trailing().as_str().unwrap_or("");
    let addition = if !has_line(PROFILES_MARKER) {
        Some((
            explanation,
            "the provider profile explanation and examples (at the end)",
        ))
    } else if !has_line(EXAMPLES_MARKER) {
        let examples = explanation
            .find(EXAMPLES_MARKER)
            .map_or("", |start| &explanation[start..]);
        Some((examples, "the example provider profiles (at the end)"))
    } else {
        None
    };
    if let Some((text, description)) = addition {
        let mut trailing = doc.trailing().as_str().unwrap_or("").to_string();
        if !trailing.is_empty() && !trailing.ends_with('\n') {
            trailing.push('\n');
        }
        trailing.push('\n');
        trailing.push_str(text.trim_start_matches('\n'));
        doc.set_trailing(trailing);
        added.push(description.to_string());
    } else {
        // The file has the examples: add the block of each provider kind that became
        // available since (the second-instance example isn't re-added on its own).
        let missing: Vec<(&str, String)> = provider_kinds()
            .into_iter()
            .filter(|(kind, _)| !has_example(content, &doc, kind))
            .filter_map(|(kind, _)| Some((kind, profile_example(kind)?)))
            .collect();
        if !missing.is_empty() {
            let mut trailing = doc.trailing().as_str().unwrap_or("").to_string();
            if !trailing.is_empty() && !trailing.ends_with('\n') {
                trailing.push('\n');
            }
            for (kind, block) in missing {
                trailing.push_str("#\n");
                trailing.push_str(&block);
                added.push(format!("the example profile for {kind} (at the end)"));
            }
            doc.set_trailing(trailing);
        }
    }
    if added.is_empty() {
        return Ok(None);
    }
    Ok(Some(Upgrade {
        content: doc.to_string(),
        added,
    }))
}

/// Whether `content` (parsed as `doc`) has an example profile for provider kind `kind`: its
/// block header line (`## <kind>: ...`, commented out as `# ## <kind>: ...` or not), a
/// commented-out `[provider_options.<kind>]` header, or a real profile of that name.
fn has_example(content: &str, doc: &DocumentMut, kind: &str) -> bool {
    let block_header = format!("## {kind}:");
    let table_header = format!("[provider_options.{kind}]");
    let real = doc
        .get(PROFILES)
        .and_then(Item::as_table_like)
        .is_some_and(|profiles| profiles.contains_key(kind));
    real || content.lines().map(str::trim).any(|line| {
        let uncommented = line.strip_prefix("# ").unwrap_or(line);
        uncommented.starts_with(&block_header)
            || line
                .trim_start_matches(|c: char| c == '#' || c.is_whitespace())
                .starts_with(&table_header)
    })
}

#[cfg(test)]
mod tests {
    use super::super::parse_config;
    use super::*;

    fn unknown(content: &str) -> Vec<UnknownKey> {
        unknown_keys(&content.parse().unwrap())
    }

    #[test]
    fn edit_distance_counts_character_edits() {
        assert_eq!(edit_distance("", ""), 0);
        assert_eq!(edit_distance("apikey", "api_key"), 1);
        assert_eq!(edit_distance("kitten", "sitting"), 3);
        assert_eq!(edit_distance("ä", "a"), 1);
    }

    #[test]
    fn a_generated_file_has_no_unknown_keys() {
        assert_eq!(unknown(&render_config(&Config::default())), vec![]);
        assert_eq!(unknown(""), vec![]);
    }

    #[test]
    fn unknown_key_with_a_suggestion_names_its_line() {
        let found = unknown("[interface]\ncopy_to_clipboard = true\nauto_hide_seconds = 5\n");
        assert_eq!(
            found,
            vec![UnknownKey {
                line: Some(3),
                what: "unknown key `auto_hide_seconds` in [interface]".into(),
                hint: Some("did you mean `auto_hide_terminal_seconds`?".into()),
            }]
        );
        assert_eq!(
            found[0].warning("tagent-cli.toml"),
            "Warning: tagent-cli.toml line 3: unknown key `auto_hide_seconds` in [interface]\n\
             \x20        (did you mean `auto_hide_terminal_seconds`?)"
        );
    }

    #[test]
    fn typos_are_suggested() {
        let found =
            unknown("[dictionary]\nspellcheck = true\n[hotkeys]\ntranslation_hotkey = \"F9\"\n");
        let hints: Vec<_> = found.iter().map(|u| u.hint.as_deref()).collect();
        assert_eq!(
            hints,
            vec![
                Some("did you mean `spell_check`?"),
                Some("did you mean `translate_hotkey`?"),
            ]
        );
    }

    #[test]
    fn unknown_key_without_a_suggestion() {
        let found = unknown("[speech]\nvolume = 11\n");
        assert_eq!(
            found,
            vec![UnknownKey {
                line: Some(2),
                what: "unknown key `volume` in [speech]".into(),
                hint: None,
            }]
        );
        assert_eq!(
            found[0].warning("tagent-cli.toml"),
            "Warning: tagent-cli.toml line 2: unknown key `volume` in [speech]"
        );
    }

    #[test]
    fn unknown_section_suggests_a_known_one() {
        let found =
            unknown("# comment\n\n[Provider]\ntranslate_provider = \"google\"\n[plugins]\nx = 1\n");
        assert_eq!(
            found,
            vec![
                UnknownKey {
                    line: Some(3),
                    what: "unknown section [Provider]".into(),
                    hint: Some("did you mean [provider]?".into()),
                },
                UnknownKey {
                    line: Some(5),
                    what: "unknown section [plugins]".into(),
                    hint: None,
                },
            ]
        );
    }

    #[test]
    fn a_key_in_the_wrong_section_names_the_right_one() {
        let found = unknown("[translation]\ncopy_to_clipboard = true\n");
        assert_eq!(
            found[0].hint.as_deref(),
            Some("`copy_to_clipboard` belongs in [interface]")
        );
        let found = unknown("[translation]\ncopy_to_clipbord = true\n");
        assert_eq!(
            found[0].hint.as_deref(),
            Some("did you mean `copy_to_clipboard` in [interface]?")
        );
    }

    #[test]
    fn a_key_outside_any_section_names_its_section() {
        let found = unknown("show_dictionary = false\n\n[speech]\n");
        assert_eq!(
            found,
            vec![UnknownKey {
                line: Some(1),
                what: "unknown key `show_dictionary` outside any section".into(),
                hint: Some("`show_dictionary` belongs in [dictionary]".into()),
            }]
        );
    }

    #[test]
    fn dotted_and_inline_keys_are_checked_with_their_lines() {
        let found = unknown("interface.copy_to_clipbord = true\nspeech = { volume = 1 }\n");
        let lines: Vec<_> = found.iter().map(|u| (u.line, u.what.as_str())).collect();
        assert_eq!(
            lines,
            vec![
                (Some(1), "unknown key `copy_to_clipbord` in [interface]"),
                (Some(2), "unknown key `volume` in [speech]"),
            ]
        );
    }

    #[test]
    fn undeclared_profile_options_are_reported() {
        let found = unknown(
            "[provider_options.deepl]\napikey = \"k\"\nTIMEOUT_SECS = \"5\"\n\n\
             [provider_options.work]\ntype = \"deepl\"\nmodel = \"x\"\n\n\
             [provider_options.mystery]\nanything = \"goes\"\n",
        );
        assert_eq!(
            found,
            vec![
                UnknownKey {
                    line: Some(2),
                    what: "unknown option `apikey` in [provider_options.deepl]".into(),
                    hint: Some("did you mean `api_key`?".into()),
                },
                UnknownKey {
                    line: Some(7),
                    what: "unknown option `model` in [provider_options.work]".into(),
                    hint: None,
                },
            ]
        );
    }

    #[test]
    fn uncommented_example_profiles_have_no_unknown_options() {
        let mut toml = render_config(&Config::default());
        for (kind, _) in provider_kinds() {
            toml = super::super::tests::uncomment_example(&toml, kind);
        }
        assert!(toml.contains("\n[provider_options.google]"), "{toml}");
        assert_eq!(unknown(&toml), vec![]);
    }

    /// The `openai` block with its prompts enabled too: `#` removed from the lines of the
    /// commented-out `translate_prompt` and `dictionary_prompt`, the way the file tells the
    /// user to.
    #[test]
    fn the_enabled_openai_prompts_have_no_unknown_options() {
        let toml =
            super::super::tests::uncomment_example(&render_config(&Config::default()), "openai");
        let toml = super::super::tests::uncomment_prompt(&toml);
        assert!(toml.contains("\ntranslate_prompt = \"\"\"\n"), "{toml}");
        assert!(toml.contains("\ndictionary_prompt = \"\"\"\n"), "{toml}");
        assert!(toml.contains("\n#response_format = \"\"\n"), "{toml}");
        assert_eq!(unknown(&toml), vec![]);
    }

    /// An old-style file: sections and keys missing, custom comments, an unknown key and a
    /// commented-out one.
    const OLD_FILE: &str = r#"# My settings
[provider]
translate_provider = "google"   # mine

[translation]
source_language = "English"
target_language = "German"

[interface]
copy_to_clipboard = true  # handy
future_key = 1

[hotkeys]
translate_hotkey = "F9"

[speech]
enable_text_to_speech = false
#   speech_provider = "google"
"#;

    /// `content` upgraded; panics when nothing was missing.
    fn upgraded(content: &str) -> Upgrade {
        upgrade(content).unwrap().expect("something to add")
    }

    #[test]
    fn upgrade_adds_what_is_missing_and_keeps_the_rest() {
        let result = upgraded(OLD_FILE);
        let text = &result.content;
        assert_eq!(
            result.added,
            vec![
                "[dictionary] (section, 3 settings)",
                "[interface] show_terminal_on_translate",
                "[interface] auto_hide_terminal_seconds",
                "[colors] (section, 8 settings)",
                "[history] (section, 2 settings)",
                "[speech] speech_hotkey",
                "[speech] enable_speech_hotkey",
                "the provider profile explanation and examples (at the end)",
            ]
        );
        // The user's values stay; everything added has its default.
        assert_eq!(
            parse_config(text).unwrap(),
            Config {
                source_language: "en".into(),
                target_language: "de".into(),
                copy_to_clipboard: true,
                translate_hotkey: "F9".into(),
                enable_text_to_speech: false,
                ..Config::default()
            }
        );
        // The user's comments, the unknown key and the commented-out key stay as they are.
        for kept in [
            "# My settings\n[provider]\ntranslate_provider = \"google\"   # mine\n",
            "[interface]\ncopy_to_clipboard = true  # handy\nfuture_key = 1\n",
            "\n#   speech_provider = \"google\"\n",
        ] {
            assert!(text.contains(kept), "{kept:?} in\n{text}");
        }
        // Added keys come with the template's comments, a missing section in its
        // template place (after [translation], not at the end).
        assert!(text.contains("future_key = 1\n\n# Show terminal window on top when translating\n"));
        let at = |needle: &str| text.find(needle).unwrap_or_else(|| panic!("{needle}"));
        assert!(at("[translation]") < at("[dictionary]"));
        assert!(at("[dictionary]") < at("[interface]"));
        assert!(at("[interface]") < at("[colors]"));
        assert!(at("[history]") < at("[hotkeys]"));
        assert!(at("\n[speech]") < at("\n# Provider profiles\n"));
        // The commented-out key isn't added back.
        assert!(!text.contains("\nspeech_provider ="), "{text}");
        // Nothing unknown was introduced, and a second run has nothing to do.
        assert_eq!(unknown(text).len(), 1);
        assert!(upgrade(text).unwrap().is_none());
        assert_eq!(new_settings_count(text), 0);
    }

    #[test]
    fn an_empty_file_becomes_a_complete_one() {
        let text = upgraded("").content;
        assert_eq!(parse_config(&text).unwrap(), Config::default());
        assert_eq!(unknown(&text), vec![]);
        assert!(text.starts_with("[provider]\n"), "{text}");
        assert!(text.contains("\n# Provider profiles\n"));
        assert!(upgrade(&text).unwrap().is_none());
        assert_eq!(new_settings_count(&text), 0);
    }

    #[test]
    fn a_generated_file_is_up_to_date() {
        let text = render_config(&Config::default());
        assert!(upgrade(&text).unwrap().is_none());
        assert_eq!(new_settings_count(&text), 0);
    }

    #[test]
    fn a_first_section_goes_before_the_others() {
        let text = upgraded("[translation]\ntarget_language = \"German\"\n").content;
        assert!(text.starts_with("[provider]\n"), "{text}");
        assert!(text.contains("\n\n[translation]\n"), "{text}");
        let added = upgraded("[translation]\ntarget_language = \"German\"\n").added;
        assert_eq!(added[0], "[provider] (section, 1 setting)");
        // Without the file introduction, which belongs to the template's first section.
        assert!(
            !text.contains("# Text Translator Configuration File"),
            "{text}"
        );
        assert_eq!(parse_config(&text).unwrap().target_language, "de");
    }

    #[test]
    fn a_file_without_a_final_newline_stays_valid() {
        let text = upgraded("[interface]\ncopy_to_clipboard = true  # no newline").content;
        assert!(parse_config(&text).unwrap().copy_to_clipboard);
        assert!(
            text.contains("copy_to_clipboard = true  # no newline\n"),
            "{text}"
        );
    }

    #[test]
    fn dotted_and_inline_sections_get_plain_keys() {
        let text = upgraded(
            "interface.copy_to_clipboard = true\nspeech = { enable_text_to_speech = false }\n",
        )
        .content;
        let config = parse_config(&text).unwrap();
        assert!(config.copy_to_clipboard);
        assert!(!config.enable_text_to_speech);
        assert!(upgrade(&text).unwrap().is_none(), "{text}");
    }

    #[test]
    fn profiles_stay_and_the_examples_follow_them() {
        let content = "[provider]\ntranslate_provider = \"work\"\n\n\
                       [provider_options.work]\ntype = \"deepl\"\napi_key = \"k\"\n";
        let text = upgraded(content).content;
        let config = parse_config(&text).unwrap();
        assert_eq!(config.translate_provider, "work");
        assert_eq!(config.provider_options.get("work").unwrap().len(), 2);
        // New sections go after [provider], before the profile.
        assert!(text.find("[speech]").unwrap() < text.find("[provider_options.work]").unwrap());
        assert!(
            text.find("[provider_options.work]").unwrap()
                < text.find("# Provider profiles").unwrap()
        );
    }

    /// A current file without the `openai` example block, as files generated before the
    /// provider existed are.
    fn file_before_openai() -> String {
        let complete = render_config(&Config::default());
        let block = format!("#\n{}", profile_example("openai").unwrap());
        assert_eq!(complete.matches(&block).count(), 1, "{complete}");
        complete.replace(&block, "")
    }

    #[test]
    fn a_file_without_a_providers_example_gets_exactly_that_block() {
        let old = file_before_openai();
        assert_eq!(new_settings_count(&old), 0);
        let result = upgraded(&old);
        assert_eq!(
            result.added,
            vec!["the example profile for openai (at the end)"]
        );
        let text = &result.content;
        assert!(text.starts_with(&old), "{text}");
        let block = profile_example("openai").unwrap();
        assert!(text.ends_with(&format!("\n#\n{block}")), "{text}");
        assert_eq!(text.matches("# [provider_options.deepl]\n").count(), 1);
        assert!(upgrade(text).unwrap().is_none(), "{text}");
        assert_eq!(unknown(text), vec![]);
        assert_eq!(parse_config(text).unwrap(), Config::default());
    }

    #[test]
    fn several_missing_examples_come_in_registry_order() {
        let complete = render_config(&Config::default());
        let mut old = complete.clone();
        for kind in ["deepl", "openai"] {
            old = old.replace(&format!("#\n{}", profile_example(kind).unwrap()), "");
        }
        let result = upgraded(&old);
        assert_eq!(
            result.added,
            vec![
                "the example profile for deepl (at the end)",
                "the example profile for openai (at the end)",
            ]
        );
        // The second-instance block stayed and isn't added again.
        assert_eq!(
            result
                .content
                .matches("[provider_options.deepl-work]")
                .count(),
            1
        );
        assert!(upgrade(&result.content).unwrap().is_none());
    }

    #[test]
    fn an_enabled_or_real_profile_counts_as_the_example() {
        let old = file_before_openai();
        for extra in [
            // The block uncommented (only its first lines matter).
            "## openai: OpenAI-compatible\n[provider_options.openai]\nendpoint = \"http://localhost:11434/v1\"\nmodel = \"m\"\n",
            // The block header, as a new file writes it, and commented out as before
            // 0.17.0+017.
            "## openai: OpenAI-compatible\n",
            "# ## openai: OpenAI-compatible\n",
            // A commented-out table of the kind.
            "#[provider_options.openai]\n",
            // A real profile named after the kind, in inline form.
            "[provider_options]\nopenai = { endpoint = \"http://localhost:11434/v1\", model = \"m\" }\n",
        ] {
            let content = format!("{old}\n{extra}");
            assert!(upgrade(&content).unwrap().is_none(), "{extra}");
        }
        // Profiles of other names don't count, even of the same kind.
        for extra in [
            "[provider_options.openai-work]\ntype = \"openai\"\nendpoint = \"http://x/v1\"\nmodel = \"m\"\n",
            "[provider_options.ollama]\ntype = \"openai\"\nendpoint = \"http://x/v1\"\nmodel = \"m\"\n",
            "# ## openaix: something\n",
        ] {
            let content = format!("{old}\n{extra}");
            let result = upgraded(&content);
            assert_eq!(
                result.added,
                vec!["the example profile for openai (at the end)"],
                "{extra}"
            );
            assert!(result.content.starts_with(&content));
            assert!(parse_config(&result.content).is_ok());
        }
    }

    #[test]
    fn a_missing_example_after_a_final_table_without_newline_stays_valid() {
        let content = format!(
            "{}\n[provider_options.work]\ntype = \"deepl\"\napi_key = \"k\"",
            file_before_openai()
        );
        let text = upgraded(&content).content;
        let config = parse_config(&text).unwrap();
        assert_eq!(config.provider_options.get("work").unwrap().len(), 2);
        assert!(text.contains("api_key = \"k\"\n#\n## openai:"), "{text}");
    }

    /// A file as 0.17.0+000 to +003 wrote it: the profiles explanation with a small
    /// example of its own, no ready-made profiles, and here one profile of the user's.
    fn file_before_the_examples() -> String {
        let complete = render_config(&Config::default());
        let cut = complete.find("#\n# Ready-made profiles").unwrap();
        format!(
            "{}# Example:\n#   [provider_options.google]\n#   timeout_secs = \"15\"\n\n\
             [provider_options.work]\ntype = \"deepl\"\napi_key = \"k\"\n",
            &complete[..cut]
        )
    }

    #[test]
    fn a_file_from_before_the_examples_gets_only_the_examples() {
        let old = file_before_the_examples();
        assert_eq!(new_settings_count(&old), 0);
        let result = upgraded(&old);
        assert_eq!(
            result.added,
            vec!["the example provider profiles (at the end)"]
        );
        let text = &result.content;
        assert!(text.starts_with(&old), "{text}");
        assert_eq!(text.matches("# Provider profiles").count(), 1);
        assert!(text.contains("\n\n# Ready-made profiles"), "{text}");
        assert!(text.contains("\n# [provider_options.deepl]\n"), "{text}");
        let config = parse_config(text).unwrap();
        assert!(config.provider_options.get("work").is_some());
        assert_eq!(
            Config {
                provider_options: Default::default(),
                ..config
            },
            Config::default()
        );
        assert_eq!(unknown(text), vec![]);
        assert!(upgrade(text).unwrap().is_none());
    }

    #[test]
    fn commented_out_keys_are_found_in_their_section() {
        let keys = commented_out_keys(
            "#top = 1\n[speech]\n  #   speech_hotkey = \"F10\"\n## enable_speech_hotkey=false\n\
             # Hotkey for text-to-speech\n# not a key: = x\n\
             [dictionary]\n# spell_check = true\n\
             # [colors]\n# error_color = \"Red\"\n",
        );
        let mut keys: Vec<_> = keys.into_iter().collect();
        keys.sort();
        let expected: Vec<(String, String)> = [
            ("", "top"),
            ("colors", "error_color"),
            ("dictionary", "spell_check"),
            ("speech", "enable_speech_hotkey"),
            ("speech", "speech_hotkey"),
        ]
        .iter()
        .map(|(s, k)| (s.to_string(), k.to_string()))
        .collect();
        assert_eq!(keys, expected);
    }

    #[test]
    fn new_settings_count_skips_commented_out_keys_of_that_section_only() {
        let complete = render_config(&Config::default());
        let without = |key: &str| -> String {
            complete
                .lines()
                .filter(|line| !line.starts_with(&format!("{key} =")))
                .map(|line| format!("{line}\n"))
                .collect()
        };
        assert_eq!(new_settings_count(&without("speech_provider")), 1);
        let commented = complete.replace("\nspeech_provider =", "\n# speech_provider =");
        assert_eq!(new_settings_count(&commented), 0);
        let indented = complete.replace("\nspeech_provider =", "\n  #   speech_provider =");
        assert_eq!(new_settings_count(&indented), 0);
        // Commented out under another section: still missing from [speech].
        let elsewhere = without("speech_provider")
            .replace("[hotkeys]\n", "[hotkeys]\n# speech_provider = \"google\"\n");
        assert_eq!(new_settings_count(&elsewhere), 1);
        assert_eq!(new_settings_count("not [valid"), 0);
    }

    #[test]
    fn a_commented_out_section_is_added_without_its_commented_keys() {
        let complete = render_config(&Config::default());
        let start = complete.find("[colors]").unwrap();
        let end = complete.find("[history]").unwrap();
        let colors: String = complete[start..end]
            .lines()
            .map(|line| format!("# {line}\n"))
            .collect();
        let content = format!("{}{colors}{}", &complete[..start], &complete[end..]);
        assert_eq!(new_settings_count(&content), 0);
        assert!(upgrade(&content).unwrap().is_none());

        let only_error_color = content.replace("# error_color =", "error_color_gone =");
        let result = upgraded(&only_error_color);
        assert!(result.added.contains(&"[colors] error_color".to_string()));
    }
}
