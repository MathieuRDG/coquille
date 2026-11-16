//! Combine les moteurs de correction : LanguageTool (toujours) et Grammalecte (si configuré,
//! pour les textes en français). Les deux sont interrogés en parallèle, puis leurs résultats
//! sont fusionnés sans doublons.

use crate::config::Config;
use crate::langtool::{self, CheckResult, Issue};
use crate::grammalecte;

fn overlaps(a: &Issue, b: &Issue) -> bool {
    a.offset < b.offset + b.length && b.offset < a.offset + a.length
}

/// Ajoute les remarques de Grammalecte à celles de LanguageTool :
/// - même passage → une seule remarque, suggestions réunies ;
/// - passages qui se chevauchent avec une suggestion commune → doublon, on garde LanguageTool ;
/// - sinon la remarque de Grammalecte est ajoutée.
pub fn merge(mut primary: Vec<Issue>, secondary: Vec<Issue>) -> Vec<Issue> {
    for g in secondary {
        if let Some(same) = primary.iter_mut().find(|p| p.offset == g.offset && p.length == g.length) {
            if same.replacements.is_empty() {
                // LanguageTool n'avait rien à proposer : le message de Grammalecte est plus utile.
                same.message = g.message;
            }
            for r in g.replacements {
                if !same.replacements.contains(&r) {
                    same.replacements.push(r);
                }
            }
            continue;
        }
        let duplicate = primary
            .iter()
            .any(|p| overlaps(p, &g) && p.replacements.iter().any(|r| g.replacements.contains(r)));
        if !duplicate {
            primary.push(g);
        }
    }
    primary.sort_by(|a, b| a.offset.cmp(&b.offset).then(b.length.cmp(&a.length)));
    primary
}

fn wants_grammalecte(cfg: &Config, language: &str) -> bool {
    !cfg.grammalecte_url.is_empty() && (language == "auto" || language.starts_with("fr"))
}

pub async fn check_all(client: &reqwest::Client, cfg: &Config, text: &str, language: &str) -> Result<CheckResult, String> {
    let lang = if language.trim().is_empty() { cfg.language.as_str() } else { language.trim() };
    let use_gl = wants_grammalecte(cfg, lang);
    let (lt, gl) = tokio::join!(langtool::check(client, cfg, text, lang), async {
        if use_gl {
            Some(grammalecte::check(client, cfg, text).await)
        } else {
            None
        }
    });
    match (lt, gl) {
        (Ok(mut result), Some(Ok(gl_issues))) => {
            // En détection automatique, on ignore Grammalecte si le texte n'est pas en français.
            if lang != "auto" || result.language_code.starts_with("fr") {
                result.issues = merge(result.issues, gl_issues);
            }
            Ok(result)
        }
        (Ok(mut result), Some(Err(e))) => {
            result.warnings.push(format!("{e} : texte vérifié par LanguageTool seul."));
            Ok(result)
        }
        (Ok(result), None) => Ok(result),
        (Err(lt_error), Some(Ok(gl_issues))) if lang != "auto" => Ok(CheckResult {
            issues: merge(Vec::new(), gl_issues),
            language: "Français".into(),
            language_code: "fr".into(),
            warnings: vec![format!("{lt_error} — seules les remarques de Grammalecte sont affichées.")],
        }),
        (Err(lt_error), _) => Err(lt_error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn issue(offset: usize, length: usize, original: &str, reps: &[&str], source: &str) -> Issue {
        Issue {
            offset,
            length,
            original: original.into(),
            message: format!("msg {source}"),
            short_message: String::new(),
            replacements: reps.iter().map(|s| s.to_string()).collect(),
            rule_id: format!("{source}-rule"),
            rule_description: String::new(),
            category: String::new(),
            kind: "grammaire".into(),
            key: 0,
            source: source.into(),
        }
    }

    #[test]
    fn merges_without_duplicates() {
        // « Les amis est là. Il faut que tu viens. »
        let lt = vec![issue(9, 3, "est", &["sont"], "languagetool"), issue(30, 5, "viens", &[], "languagetool")];
        let gl = vec![
            issue(9, 3, "est", &["sont", "étaient"], "grammalecte"),     // même passage
            issue(4, 8, "amis est", &["amis sont"], "grammalecte"),      // chevauche, suggestion différente
            issue(5, 7, "mis est", &["sont"], "grammalecte"),            // chevauche + suggestion commune → doublon
            issue(30, 5, "viens", &["viennes"], "grammalecte"),          // même passage, LT sans suggestion
            issue(40, 3, "foo", &["bar"], "grammalecte"),                // nouveau
        ];
        let merged = merge(lt, gl);
        let summary: Vec<(usize, &str, Vec<&str>)> =
            merged.iter().map(|i| (i.offset, i.source.as_str(), i.replacements.iter().map(String::as_str).collect())).collect();
        assert_eq!(
            summary,
            vec![
                (4, "grammalecte", vec!["amis sont"]),
                (9, "languagetool", vec!["sont", "étaient"]),
                (30, "languagetool", vec!["viennes"]),
                (40, "grammalecte", vec!["bar"]),
            ]
        );
        assert_eq!(merged[2].message, "msg grammalecte", "message de Grammalecte repris quand LT n'avait pas de suggestion");
    }

    #[test]
    fn grammalecte_only_for_french() {
        let cfg = Config { grammalecte_url: "http://gl:8080".into(), ..Default::default() };
        assert!(wants_grammalecte(&cfg, "fr"));
        assert!(wants_grammalecte(&cfg, "fr-CA"));
        assert!(wants_grammalecte(&cfg, "auto"));
        assert!(!wants_grammalecte(&cfg, "en-US"));
        assert!(!wants_grammalecte(&Config::default(), "fr"));
    }
}
