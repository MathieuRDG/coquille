//! Client du serveur Grammalecte (https://grammalecte.net), second moteur spécialisé en français :
//! accords, conjugaison, participes passés, modes verbaux, confusions d'homonymes.
//!
//! API : `POST /gc_text/fr` (formulaire `text`, `options`). Le serveur découpe le texte en
//! paragraphes (sauts de ligne) et renvoie, pour chaque paragraphe fautif, des positions
//! relatives au paragraphe, comptées en caractères Unicode (points de code Python).
//! On les convertit en positions UTF-16 dans le texte complet, comme partout dans Coquille.

use crate::config::Config;
use crate::langtool::Issue;
use serde::Deserialize;

/// Préfixe des identifiants de règles Grammalecte (pour les distinguer de celles de LanguageTool).
pub const RULE_PREFIX: &str = "grammalecte:";

/// Options envoyées à chaque requête : la typographie est déjà vérifiée par LanguageTool, et
/// l'apostrophe typographique (’) signalerait chaque « j'ai » tapé au clavier.
const OPTIONS: &str = r#"{"typo":false,"apos":false,"esp":false,"tab":false,"nbsp":false,"unit":false,"nf":false,"num":false,"chim":false}"#;

const MAX_SUGGESTIONS: usize = 8;

#[derive(Debug, Deserialize)]
struct GlResponse {
    #[serde(default)]
    data: Vec<GlParagraph>,
    #[serde(default)]
    version: String,
}

#[derive(Debug, Deserialize)]
struct GlParagraph {
    #[serde(rename = "iParagraph")]
    paragraph: usize,
    #[serde(rename = "lGrammarErrors", default)]
    grammar: Vec<GlError>,
}

#[derive(Debug, Deserialize)]
struct GlError {
    #[serde(rename = "nStart")]
    start: usize,
    #[serde(rename = "nEnd")]
    end: usize,
    #[serde(rename = "sRuleId", default)]
    rule_id: String,
    #[serde(rename = "sType", default)]
    kind: String,
    #[serde(rename = "sMessage", default)]
    message: String,
    #[serde(rename = "aSuggestions", default)]
    suggestions: Vec<String>,
}

/// Catégorie d'option Grammalecte → type de faute affiché par Coquille.
fn classify(option: &str) -> &'static str {
    match option {
        "typo" | "apos" | "esp" | "tab" | "nbsp" | "unit" | "nf" | "num" | "chim" | "liga" | "maj" | "minis"
        | "tu" | "virg" | "poncfin" | "eepi" | "mapos" | "ocr" => "typographie",
        "bs" | "pleo" | "eleu" | "neg" | "redon1" | "redon2" | "mc" => "style",
        _ => "grammaire",
    }
}

fn label(option: &str) -> &'static str {
    match option {
        "conj" => "Conjugaison",
        "infi" => "Infinitif",
        "ppas" => "Participes passés",
        "imp" => "Impératif",
        "inte" => "Interrogatif",
        "vmode" => "Modes verbaux",
        "gn" => "Accords (genre et nombre)",
        "conf" => "Confusions",
        "loc" => "Locutions",
        "maj" => "Majuscules",
        "tu" => "Traits d'union",
        "virg" => "Virgules",
        "bs" => "Populaire",
        "pleo" => "Pléonasmes",
        "eleu" => "Élisions et euphonies",
        "date" => "Dates",
        _ => "Grammalecte",
    }
}

/// Découpe le texte en paragraphes comme Grammalecte (\r\n, \r ou \n) et renvoie, pour chacun,
/// sa position de départ en UTF-16 dans le texte d'origine et son contenu.
fn paragraphs(text: &str) -> Vec<(usize, &str)> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let (mut start_byte, mut start_u16, mut u16_pos) = (0usize, 0usize, 0usize);
    let mut chars = text.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        if c == '\r' || c == '\n' {
            out.push((start_u16, &text[start_byte..i]));
            let mut sep_len = 1;
            if c == '\r' && bytes.get(i + 1) == Some(&b'\n') {
                chars.next();
                sep_len = 2;
            }
            u16_pos += sep_len;
            start_byte = i + sep_len;
            start_u16 = u16_pos;
        } else {
            u16_pos += c.len_utf16();
        }
    }
    out.push((start_u16, &text[start_byte..]));
    out
}

/// Position en points de code → position en unités UTF-16 dans `s`.
fn cp_to_utf16(s: &str, cp: usize) -> Option<usize> {
    let mut units = 0;
    let mut chars = s.chars();
    for _ in 0..cp {
        units += chars.next()?.len_utf16();
    }
    Some(units)
}

fn convert(text: &str, resp: GlResponse, cfg: &Config) -> Vec<Issue> {
    let paras = paragraphs(text);
    let mut issues = Vec::new();
    for p in resp.data {
        let Some(&(para_start, para)) = p.paragraph.checked_sub(1).and_then(|i| paras.get(i)) else { continue };
        for e in p.grammar {
            if e.end <= e.start {
                continue;
            }
            let rule_id = format!("{RULE_PREFIX}{}", e.rule_id);
            if cfg.disabled_rules.contains(&rule_id) {
                continue;
            }
            let (Some(s16), Some(e16)) = (cp_to_utf16(para, e.start), cp_to_utf16(para, e.end)) else { continue };
            let original: String = para.chars().skip(e.start).take(e.end - e.start).collect();
            let mut replacements: Vec<String> = Vec::new();
            for r in e.suggestions {
                if r != original && !replacements.contains(&r) {
                    replacements.push(r);
                }
                if replacements.len() >= MAX_SUGGESTIONS {
                    break;
                }
            }
            issues.push(Issue {
                offset: para_start + s16,
                length: e16 - s16,
                original,
                message: e.message,
                short_message: String::new(),
                replacements,
                rule_description: format!("Grammalecte — {}", label(&e.kind)),
                rule_id,
                category: label(&e.kind).into(),
                kind: classify(&e.kind).into(),
                key: 0,
                source: "grammalecte".into(),
            });
        }
    }
    issues
}

fn describe_error(e: &reqwest::Error, server: &str) -> String {
    if e.is_timeout() {
        format!("Grammalecte : délai dépassé en contactant {server}")
    } else if e.is_connect() {
        format!("Grammalecte injoignable ({server})")
    } else {
        format!("Grammalecte : erreur réseau avec {server} : {e}")
    }
}

async fn request(client: &reqwest::Client, base: &str, text: &str) -> Result<GlResponse, String> {
    let url = format!("{base}/gc_text/fr");
    let resp = client
        .post(&url)
        .form(&[("text", text), ("options", OPTIONS)])
        .send()
        .await
        .map_err(|e| describe_error(&e, base))?;
    if !resp.status().is_success() {
        return Err(format!("Grammalecte a répondu {} ({url})", resp.status()));
    }
    resp.json::<GlResponse>()
        .await
        .map_err(|_| format!("{url} ne ressemble pas à un serveur Grammalecte"))
}

/// Vérifie `text` avec Grammalecte. Les fautes d'orthographe sont laissées à LanguageTool
/// (même dictionnaire d'origine, et LanguageTool fournit les suggestions).
pub async fn check(client: &reqwest::Client, cfg: &Config, text: &str) -> Result<Vec<Issue>, String> {
    if cfg.grammalecte_url.is_empty() || text.trim().is_empty() {
        return Ok(Vec::new());
    }
    let resp = request(client, &cfg.grammalecte_url, text).await?;
    Ok(convert(text, resp, cfg))
}

/// Teste la connexion et renvoie la version du serveur.
pub async fn test(client: &reqwest::Client, base: &str) -> Result<String, String> {
    let resp = request(client, base, "Les amis est là.").await?;
    let found = resp.data.iter().any(|p| !p.grammar.is_empty());
    let version = if resp.version.is_empty() { "?".into() } else { resp.version };
    Ok(format!(
        "Connexion réussie à Grammalecte {version} ({base}){}",
        if found { " : l'erreur de test « Les amis est là » est bien détectée." } else { "." }
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Réponse réelle du serveur Grammalecte 2.3.0 (abrégée) pour le texte de `TEXT`.
    const TEXT: &str = "Hello,\r\nje t'ai renvoyer un mdp. 😀 Les amis est là.\nIl faut que tu viens.";
    const RESPONSE: &str = r##"{ "program": "grammalecte-fr", "version": "2.3.0", "lang": "fr", "error": "", "data" : [
      {"iParagraph": 2, "lGrammarErrors": [
        {"nStart": 8, "nEnd": 16, "sLineId": "#1", "sRuleId": "gv1__ppas_n_m_t_l_avoir__b5_a1_1", "sType": "ppas", "aColor": null,
         "sMessage": "Confusion : employez un participe passé.", "aSuggestions": ["renvoyé"], "URL": ""},
        {"nStart": 36, "nEnd": 39, "sLineId": "#2", "sRuleId": "gv2__conj_les_nom__b1_a2_1", "sType": "conj", "aColor": null,
         "sMessage": "Accord avec “Les amis…” incorrect.", "aSuggestions": ["sont", "est"], "URL": ""}],
       "lSpellingErrors": [{"i": 5, "sType": "WORD", "sValue": "mdp", "nStart": 20, "nEnd": 23}]},
      {"iParagraph": 3, "lGrammarErrors": [
        {"nStart": 15, "nEnd": 20, "sLineId": "#3", "sRuleId": "gv2__vmode_verbes_que_subjonctif__b1_a1_1", "sType": "vmode", "aColor": null,
         "sMessage": "Après “faut que”, ce verbe devrait être au subjonctif.", "aSuggestions": ["viennes", "vinsses"], "URL": ""}],
       "lSpellingErrors": []}
    ]}"##;

    fn slice(text: &str, i: &Issue) -> String {
        let u: Vec<u16> = text.encode_utf16().collect();
        String::from_utf16_lossy(&u[i.offset..i.offset + i.length])
    }

    #[test]
    fn converts_paragraph_offsets_to_utf16() {
        let resp: GlResponse = serde_json::from_str(RESPONSE).unwrap();
        let issues = convert(TEXT, resp, &Config::default());
        assert_eq!(issues.len(), 3);
        // Paragraphe 2 après « \r\n » (2 unités) ; l'emoji compte 1 point de code mais 2 unités UTF-16.
        assert_eq!(slice(TEXT, &issues[0]), "renvoyer");
        assert_eq!(issues[0].original, "renvoyer");
        assert_eq!(issues[0].kind, "grammaire");
        assert_eq!(issues[0].source, "grammalecte");
        assert_eq!(slice(TEXT, &issues[1]), "est");
        assert_eq!(issues[1].replacements, vec!["sont"], "la suggestion identique au texte est retirée");
        assert_eq!(slice(TEXT, &issues[2]), "viens");
        assert_eq!(issues[2].category, "Modes verbaux");
    }

    #[test]
    fn disabled_rules_are_filtered() {
        let resp: GlResponse = serde_json::from_str(RESPONSE).unwrap();
        let cfg = Config {
            disabled_rules: vec![format!("{RULE_PREFIX}gv2__conj_les_nom__b1_a2_1")],
            ..Default::default()
        };
        let issues = convert(TEXT, resp, &cfg);
        assert!(issues.iter().all(|i| i.original != "est"));
        assert_eq!(issues.len(), 2);
    }

    /// Test contre un vrai serveur : GRAMMALECTE_URL=http://127.0.0.1:8080 cargo test -- --ignored
    #[test]
    #[ignore]
    fn live_server() {
        let url = std::env::var("GRAMMALECTE_URL").expect("GRAMMALECTE_URL");
        let cfg = Config { grammalecte_url: url.clone(), ..Default::default() };
        let client = crate::langtool::http_client();
        let text = "Hello,\r\nje t'ai renvoyer un mdp. 😀 Il faut que tu viens.\nLa voiture que j'ai acheté est rouge.";
        let issues = tauri::async_runtime::block_on(check(&client, &cfg, text)).unwrap();
        let found: Vec<(String, String)> =
            issues.iter().map(|i| (slice(text, i), i.replacements.first().cloned().unwrap_or_default())).collect();
        println!("{found:?}");
        for (word, fix) in [("renvoyer", "renvoyé"), ("viens", "viennes"), ("acheté", "achetée")] {
            assert!(found.iter().any(|(w, f)| w == word && f == fix), "{word} → {fix} attendu dans {found:?}");
        }
        assert!(issues.iter().all(|i| slice(text, i) == i.original), "positions UTF-16 exactes");
        let msg = tauri::async_runtime::block_on(test(&client, &url)).unwrap();
        println!("{msg}");
    }

    #[test]
    fn paragraph_split_matches_grammalecte() {
        let p = paragraphs("a\r\nbé\rc\n\nd");
        assert_eq!(p, vec![(0, "a"), (3, "bé"), (6, "c"), (8, ""), (9, "d")]);
        assert_eq!(cp_to_utf16("😀ab", 2), Some(3));
        assert_eq!(cp_to_utf16("ab", 3), None);
    }
}
