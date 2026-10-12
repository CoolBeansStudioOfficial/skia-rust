// Copyright 2026 The skia-rust Authors
// Use of this source code is governed by a BSD-style license that can be found in the LICENSE file.

//! `uloc_forLanguageTag` and `uloc_getDefault`, and ICU's view of a locale ID (`Locale::getLanguage`,
//! `getKeywordValue`) that the break-iterator factory reads.
//!
//! Skia converts a BCP 47 tag with `uloc_forLanguageTag` before `ubrk_open`
//! (`SkUnicode_icu.cpp#L235-L248`); what reaches the break rules is the language, script, region and
//! the `lb` / `lw` / `ss` keywords. `oracle/unicode-diff`'s `misc` section compares the conversion
//! with Chromium's ICU.
//!
//! Not ported: ICU's full BCP 47 → legacy key/type tables (`keyTypeData`): of the `-u-` keys, `ca`,
//! `co`, `cu`, `nu` and `tz` are renamed to `calendar`, `collation`, `currency`, `numbers` and
//! `timezone`, and of the types only `co-phonebk`, `co-dict`, `co-trad` and `ca-gregory`; any other
//! key or type is kept as written (which is ICU's result for `lb`, `lw`, `ss` and most others).
//! Grandfathered tags (`i-klingon`) are not canonicalized.

/// `uloc_getDefault()`.
///
/// ICU derives the default from the process environment (`LANG`, the Windows user locale); the
/// shim has no global state, and fixes it to `en_US`, ICU's result on the hosts the oracle and the
/// bots run on. Every break rule of `en_US` is the root rule.
#[doc(alias = "uloc_getDefault")]
#[must_use]
pub fn uloc_get_default() -> &'static str {
    "en_US"
}

/// The result of [`uloc_for_language_tag`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ForLanguageTag {
    /// The ICU locale ID (`localeID`).
    pub locale_id: String,
    /// `*parsedLength`: the length of the tag prefix that was parsed.
    pub parsed_length: usize,
}

fn is_alpha(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphabetic())
}

fn is_alnum(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_alphanumeric())
}

fn is_digit(s: &str) -> bool {
    s.bytes().all(|b| b.is_ascii_digit())
}

/// `ultag_isLanguageSubtag`.
fn is_language(s: &str) -> bool {
    (2..=8).contains(&s.len()) && s.len() != 4 && is_alpha(s)
}

/// `ultag_isExtlangSubtag`.
fn is_extlang(s: &str) -> bool {
    s.len() == 3 && is_alpha(s)
}

/// `ultag_isScriptSubtag`.
fn is_script(s: &str) -> bool {
    s.len() == 4 && is_alpha(s)
}

/// `ultag_isRegionSubtag`.
fn is_region(s: &str) -> bool {
    (s.len() == 2 && is_alpha(s)) || (s.len() == 3 && is_digit(s))
}

/// `ultag_isVariantSubtag`.
fn is_variant(s: &str) -> bool {
    ((5..=8).contains(&s.len()) && is_alnum(s))
        || (s.len() == 4 && s.as_bytes()[0].is_ascii_digit() && is_alnum(s))
}

/// `ultag_isExtensionSubtag`.
fn is_extension(s: &str) -> bool {
    (2..=8).contains(&s.len()) && is_alnum(s)
}

/// `ultag_isPrivateuseValueSubtag`.
fn is_private_use(s: &str) -> bool {
    (1..=8).contains(&s.len()) && is_alnum(s)
}

/// The legacy key of a `-u-` key (`uloc_toLegacyKey`), for the keys of the module documentation.
fn legacy_key(key: &str) -> &str {
    match key {
        "ca" => "calendar",
        "co" => "collation",
        "cu" => "currency",
        "nu" => "numbers",
        "tz" => "timezone",
        _ => key,
    }
}

/// The legacy type of a `-u-` type (`uloc_toLegacyType`), for the types of the module documentation.
fn legacy_type<'a>(key: &str, value: &'a str) -> &'a str {
    match (key, value) {
        ("co", "phonebk") => "phonebook",
        ("co", "dict") => "dictionary",
        ("co", "trad") => "traditional",
        ("ca", "gregory") => "gregorian",
        _ => value,
    }
}

/// Where the parser is in the tag: each part may only follow the ones before it.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Part {
    Language,
    Extlang1,
    Extlang2,
    Extlang3,
    Script,
    Region,
    Variant,
    Extension,
    PrivateUse,
}

// Port of: icu4c/source/common/uloc_tag.cpp ultag_parse and uloc_forLanguageTag
// (chromium/deps/icu d578f2e8), for the parts of the module documentation.
/// `uloc_forLanguageTag(langtag, localeID, capacity, &parsedLength, &status)`.
#[doc(alias = "uloc_forLanguageTag")]
#[must_use]
#[allow(clippy::too_many_lines)] // one parser, in the order of ultag_parse
pub fn uloc_for_language_tag(tag: &str) -> ForLanguageTag {
    let subtags: Vec<&str> = tag.split('-').collect();
    let mut language = String::new();
    let mut script = String::new();
    let mut region = String::new();
    let mut variants: Vec<String> = Vec::new();
    // (key, value) keywords, sorted by key at the end.
    let mut keywords: Vec<(String, String)> = Vec::new();
    let mut parsed = 0usize;
    let mut next = Part::Language;
    let mut i = 0;
    // The end of the subtag `i` in `tag`.
    let end_of =
        |i: usize| -> usize { subtags[..=i].iter().map(|s| s.len() + 1).sum::<usize>() - 1 };
    while i < subtags.len() {
        let s = subtags[i];
        let lower = s.to_ascii_lowercase();
        if next == Part::Language {
            if is_language(s) {
                language = if lower == "und" { String::new() } else { lower };
                next = if s.len() <= 3 {
                    Part::Extlang1
                } else {
                    Part::Script
                };
                parsed = end_of(i);
                i += 1;
                continue;
            }
            if lower != "x" {
                break;
            }
            next = Part::PrivateUse;
        }
        if next <= Part::Extlang3 && next >= Part::Extlang1 && is_extlang(s) {
            // The first extlang replaces the language in the locale ID (`zh-yue` is `yue`).
            if next == Part::Extlang1 {
                language = lower;
            }
            next = match next {
                Part::Extlang1 => Part::Extlang2,
                Part::Extlang2 => Part::Extlang3,
                _ => Part::Script,
            };
            parsed = end_of(i);
            i += 1;
            continue;
        }
        if next <= Part::Script && is_script(s) {
            script = format!(
                "{}{}",
                s[..1].to_ascii_uppercase(),
                s[1..].to_ascii_lowercase()
            );
            next = Part::Region;
            parsed = end_of(i);
            i += 1;
            continue;
        }
        if next <= Part::Region && is_region(s) {
            region = s.to_ascii_uppercase();
            next = Part::Variant;
            parsed = end_of(i);
            i += 1;
            continue;
        }
        if next <= Part::Variant && is_variant(s) {
            variants.push(s.to_ascii_uppercase());
            next = Part::Variant;
            parsed = end_of(i);
            i += 1;
            continue;
        }
        if s.len() == 1 && is_alnum(s) && lower != "x" && next <= Part::Extension {
            // An extension: the singleton and at least one subtag of 2 to 8 alphanumerics.
            let mut j = i + 1;
            while j < subtags.len() && is_extension(subtags[j]) && subtags[j].len() > 1 {
                j += 1;
            }
            if j == i + 1 {
                break;
            }
            let values: Vec<String> = subtags[i + 1..j]
                .iter()
                .map(|v| v.to_ascii_lowercase())
                .collect();
            if lower == "u" {
                let mut attributes = Vec::new();
                let mut k = 0;
                while k < values.len() && values[k].len() > 2 {
                    attributes.push(values[k].clone());
                    k += 1;
                }
                if !attributes.is_empty() {
                    keywords.push(("attribute".to_owned(), attributes.join("-")));
                }
                while k < values.len() {
                    let key = values[k].clone();
                    k += 1;
                    let mut types = Vec::new();
                    while k < values.len() && values[k].len() > 2 {
                        types.push(values[k].clone());
                        k += 1;
                    }
                    let value = if types.is_empty() {
                        "yes".to_owned()
                    } else {
                        legacy_type(&key, &types.join("-")).to_owned()
                    };
                    keywords.push((legacy_key(&key).to_owned(), value));
                }
            } else {
                keywords.push((lower, values.join("-")));
            }
            next = Part::Extension;
            parsed = end_of(j - 1);
            i = j;
            continue;
        }
        if lower == "x" {
            let mut j = i + 1;
            while j < subtags.len() && is_private_use(subtags[j]) {
                j += 1;
            }
            if j == i + 1 {
                break;
            }
            let values: Vec<String> = subtags[i + 1..j]
                .iter()
                .map(|v| v.to_ascii_lowercase())
                .collect();
            keywords.push(("x".to_owned(), values.join("-")));
            parsed = end_of(j - 1);
        }
        break;
    }
    let mut id = language;
    if !script.is_empty() {
        id.push('_');
        id.push_str(&script);
    }
    if !region.is_empty() {
        id.push('_');
        id.push_str(&region);
    }
    if !variants.is_empty() {
        if region.is_empty() {
            id.push('_');
        }
        for v in &variants {
            id.push('_');
            id.push_str(v);
        }
    }
    if !keywords.is_empty() {
        keywords.sort();
        id.push('@');
        let joined: Vec<String> = keywords.iter().map(|(k, v)| format!("{k}={v}")).collect();
        id.push_str(&joined.join(";"));
    }
    ForLanguageTag {
        locale_id: id,
        parsed_length: parsed,
    }
}

/// The parts of an ICU locale ID the break-iterator factory reads.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct LocaleParts {
    /// `Locale::getLanguage`, lowercase.
    pub language: String,
    /// `Locale::getScript`.
    pub script: String,
    /// `Locale::getCountry`.
    pub region: String,
    /// The `@key=value;...` keywords, keys lowercase.
    pub keywords: Vec<(String, String)>,
}

impl LocaleParts {
    /// Splits an ICU locale ID (`lang_Script_REGION_VARIANT@k=v;...`; `-` is accepted as `_`).
    pub(crate) fn parse(id: &str) -> Self {
        let (base, keywords) = id.split_once('@').unwrap_or((id, ""));
        let mut parts = base.split(['_', '-']);
        let mut out = LocaleParts {
            language: parts.next().unwrap_or("").to_ascii_lowercase(),
            ..LocaleParts::default()
        };
        let rest: Vec<&str> = parts.collect();
        let mut k = 0;
        if rest.get(k).is_some_and(|s| is_script(s)) {
            rest[k].clone_into(&mut out.script);
            k += 1;
        }
        if rest.get(k).is_some_and(|s| is_region(s)) {
            out.region = rest[k].to_ascii_uppercase();
        }
        for kv in keywords.split(';').filter(|s| !s.is_empty()) {
            if let Some((key, value)) = kv.split_once('=') {
                out.keywords
                    .push((key.trim().to_ascii_lowercase(), value.trim().to_owned()));
            }
        }
        out
    }

    /// `Locale::getKeywordValue(key)`, or `None` when the keyword is absent.
    pub(crate) fn keyword(&self, key: &str) -> Option<&str> {
        self.keywords
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}
