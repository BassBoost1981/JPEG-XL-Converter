//! Text lookup for the messages the engine produces, from the same catalogs the
//! frontend uses (`src/i18n/de.json`, `en.json`). Substitution follows
//! `translate` in `src/i18n/index.ts`: one pass over the template, an unknown key
//! gives the key itself, a missing parameter keeps its `{placeholder}`, a nested
//! message is translated in the same language.

use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

use super::msg::{Msg, MsgParam};
use super::types::Lang;

type Catalog = HashMap<String, String>;

fn parse(file: &str, json: &str) -> Catalog {
    serde_json::from_str(json)
        .unwrap_or_else(|e| panic!("{file} is not a flat JSON object of strings: {e}"))
}

fn catalog(lang: Lang) -> &'static Catalog {
    static DE: OnceLock<Catalog> = OnceLock::new();
    static EN: OnceLock<Catalog> = OnceLock::new();
    match lang {
        Lang::De => DE.get_or_init(|| parse("de.json", include_str!("../../../src/i18n/de.json"))),
        Lang::En => EN.get_or_init(|| parse("en.json", include_str!("../../../src/i18n/en.json"))),
    }
}

/// The catalog text for `key`, without substituting anything; the key itself if there is none.
pub fn key(lang: Lang, key: &str) -> String {
    catalog(lang)
        .get(key)
        .cloned()
        .unwrap_or_else(|| key.to_owned())
}

/// The text of `msg` in `lang`, nested messages included.
pub fn text(lang: Lang, msg: &Msg) -> String {
    match catalog(lang).get(&msg.key) {
        Some(template) => fill(lang, template, &msg.params),
        None => msg.key.clone(),
    }
}

/// Replaces each `{name}` (ASCII letters, digits, `_`) once; anything else stays as written.
fn fill(lang: Lang, template: &str, params: &BTreeMap<String, MsgParam>) -> String {
    let mut out = String::with_capacity(template.len());
    let mut rest = template;
    while let Some(open) = rest.find('{') {
        out.push_str(&rest[..open]);
        let after = &rest[open + 1..];
        let name_len = after
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'_')
            .count();
        if name_len > 0 && after[name_len..].starts_with('}') {
            match params.get(&after[..name_len]) {
                Some(MsgParam::Text(value)) => out.push_str(value),
                Some(MsgParam::Msg(inner)) => out.push_str(&text(lang, inner)),
                None => out.push_str(&rest[open..open + name_len + 2]),
            }
            rest = &after[name_len + 1..];
        } else {
            out.push('{');
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::msg::keys;

    #[test]
    fn every_key_has_text_in_both_languages() {
        for k in keys::ALL {
            assert_ne!(key(Lang::De, k), *k, "no German text for {k}");
            assert_ne!(key(Lang::En, k), *k, "no English text for {k}");
        }
    }

    #[test]
    fn every_rust_side_catalog_key_has_a_constant() {
        const RUST_GROUPS: [&str; 6] = ["note.", "error.", "reason.", "tool.", "preview.", "log."];
        for catalog_key in catalog(Lang::De).keys() {
            let rust_side = RUST_GROUPS.iter().any(|g| catalog_key.starts_with(g))
                || catalog_key == "banner.toolsMissing";
            if rust_side {
                assert!(
                    keys::ALL.contains(&catalog_key.as_str()),
                    "catalog key {catalog_key} has no constant in keys::ALL"
                );
            }
        }
    }

    #[test]
    fn key_gives_the_key_itself_when_missing() {
        assert_eq!(key(Lang::En, "does.not.exist"), "does.not.exist");
        assert_eq!(key(Lang::De, ""), "");
        assert_eq!(
            key(Lang::De, "note.keptTooLarge"),
            "Original behalten: zu groß für den Papierkorb"
        );
        assert_eq!(
            key(Lang::En, "note.keptTooLarge"),
            "Original kept: too large for the Recycle Bin"
        );
    }

    #[test]
    fn text_substitutes_params_once() {
        assert_eq!(
            text(Lang::De, &Msg::new("note.keptNoRecycleBin")),
            "Original behalten: kein Papierkorb auf diesem Laufwerk"
        );
        assert_eq!(
            text(Lang::En, &Msg::new("error.targetExists")),
            "Target file already exists"
        );
        let space = Msg::new("footer.noSpace").with("drive", "D:");
        assert_eq!(text(Lang::En, &space), "No space left on drive D:");
        assert_eq!(text(Lang::De, &space), "Kein Platz mehr auf Laufwerk D:");

        // A value that itself looks like a placeholder is not substituted again.
        let echo = Msg::new("footer.noSpace").with("drive", "{drive}");
        assert_eq!(text(Lang::En, &echo), "No space left on drive {drive}");
        let swapped = Msg::new("footer.doneOf")
            .with("done", "{total}")
            .with("total", "5");
        assert_eq!(text(Lang::De, &swapped), "{total} von 5 fertig");
    }

    #[test]
    fn text_translates_nested_messages() {
        let msg =
            Msg::new("note.keptRecycleFailed").with_msg("reason", Msg::new("reason.readOnly"));
        assert_eq!(
            text(Lang::De, &msg),
            "Original behalten: Papierkorb nicht möglich (Datei ist schreibgeschützt)"
        );
        assert_eq!(
            text(Lang::En, &msg),
            "Original kept: Recycle Bin not possible (File is read-only)"
        );

        let with_code = Msg::new("note.keptRecycleFailed").with_msg(
            "reason",
            Msg::new("reason.windowsError").with("code", "0x80270058"),
        );
        assert_eq!(
            text(Lang::De, &with_code),
            "Original behalten: Papierkorb nicht möglich (Windows-Fehler 0x80270058)"
        );
        assert_eq!(
            text(Lang::En, &with_code),
            "Original kept: Recycle Bin not possible (Windows error 0x80270058)"
        );
    }

    #[test]
    fn unknown_key_gives_the_key_and_missing_param_keeps_its_placeholder() {
        assert_eq!(
            text(Lang::En, &Msg::new("does.not.exist")),
            "does.not.exist"
        );
        assert_eq!(
            text(Lang::En, &Msg::new("does.not.exist").with("a", "b")),
            "does.not.exist"
        );
        assert!(text(Lang::En, &Msg::new("footer.noSpace")).contains("{drive}"));
        let unrelated = Msg::new("footer.noSpace").with("other", "x");
        assert_eq!(text(Lang::En, &unrelated), "No space left on drive {drive}");
    }

    #[test]
    fn placeholders_follow_the_typescript_rules() {
        let params = BTreeMap::from([
            ("x".to_owned(), MsgParam::Text("1".to_owned())),
            ("a_1".to_owned(), MsgParam::Text("two".to_owned())),
            ("ä".to_owned(), MsgParam::Text("umlaut".to_owned())),
        ]);
        let fill = |template: &str| fill(Lang::En, template, &params);
        assert_eq!(fill("{x}{a_1}"), "1two");
        assert_eq!(fill("{{x}"), "{1");
        assert_eq!(fill("{x}}"), "1}");
        assert_eq!(fill("{}"), "{}");
        assert_eq!(fill("{x"), "{x");
        assert_eq!(fill("x}"), "x}");
        assert_eq!(
            fill("{ä}"),
            "{ä}",
            "placeholder names are ASCII word characters only"
        );
        assert_eq!(fill("{ x }"), "{ x }");
        assert_eq!(fill("{y}"), "{y}");
    }

    #[test]
    fn lang_serde() {
        assert_eq!(serde_json::to_string(&Lang::En).unwrap(), r#""en""#);
        assert_eq!(serde_json::to_string(&Lang::De).unwrap(), r#""de""#);
        assert_eq!(serde_json::from_str::<Lang>(r#""en""#).unwrap(), Lang::En);
        assert_eq!(serde_json::from_str::<Lang>(r#""de""#).unwrap(), Lang::De);
        assert!(serde_json::from_str::<Lang>(r#""fr""#).is_err());
        assert!(serde_json::from_str::<Lang>(r#""EN""#).is_err());
    }
}
