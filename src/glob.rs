/// Module glob : expansion des wildcards *, ?, [...] dans les arguments.
///
/// Transforme les patterns avant l'exécution :
///   *.rs          → main.rs lib.rs executor.rs ...
///   src/*.rs      → src/main.rs src/lib.rs ...
///   file[0-9].txt → file0.txt file1.txt ...
///   ?argo.toml    → Cargo.toml
///
/// Si aucun fichier ne correspond, le pattern est laissé tel quel (comportement bash).

use std::fs;
use std::path::{Path, PathBuf};

/// Expand tous les arguments d'une commande : remplace les patterns glob par
/// la liste des fichiers correspondants.
pub fn expand_args(argv: &[String]) -> Vec<String> {
    let mut result = Vec::new();
    for arg in argv {
        if needs_expansion(arg) {
            let mut matches = expand_pattern(arg);
            if matches.is_empty() {
                // Aucun match : garder le pattern littéral (bash behavior)
                result.push(arg.clone());
            } else {
                matches.sort();
                result.extend(matches);
            }
        } else {
            result.push(arg.clone());
        }
    }
    result
}

/// Vérifie si un argument contient un wildcard non échappé.
fn needs_expansion(s: &str) -> bool {
    let bytes = s.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2, // skip escaped char
            b'*' | b'?' | b'[' => return true,
            _ => i += 1,
        }
    }
    false
}

/// Expand un pattern glob en liste de chemins correspondants.
pub fn expand_pattern(pattern: &str) -> Vec<String> {
    // Séparer la partie répertoire de la partie nom
    let (dir, file_pattern) = split_dir_pattern(pattern);

    let search_dir = if dir.is_empty() { Path::new(".") } else { Path::new(&dir) };

    let mut matches = Vec::new();

    if let Ok(entries) = fs::read_dir(search_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();

            // Les fichiers cachés (.xxx) ne sont pas matchés par * sauf si
            // le pattern commence explicitement par un point
            if name_str.starts_with('.') && !file_pattern.starts_with('.') {
                continue;
            }

            if glob_match(&file_pattern, &name_str) {
                let full_path = if dir.is_empty() {
                    name_str.to_string()
                } else {
                    format!("{}/{}", dir.trim_end_matches('/'), name_str)
                };
                matches.push(full_path);
            }
        }
    }

    matches
}

/// Sépare "src/foo*.rs" en ("src", "foo*.rs").
fn split_dir_pattern(pattern: &str) -> (String, String) {
    // Trouver le dernier '/' avant le premier wildcard
    let first_wild = pattern.find(|c| c == '*' || c == '?' || c == '[')
        .unwrap_or(pattern.len());

    let prefix = &pattern[..first_wild];
    if let Some(slash_pos) = prefix.rfind('/') {
        let dir = pattern[..slash_pos].to_string();
        let file = pattern[slash_pos + 1..].to_string();
        (dir, file)
    } else {
        (String::new(), pattern.to_string())
    }
}

/// Vérifie si un nom de fichier correspond à un pattern glob.
/// Supporte : * (zéro ou plus de caractères), ? (un caractère), [abc] [a-z] [!abc]
pub fn glob_match(pattern: &str, name: &str) -> bool {
    glob_match_bytes(pattern.as_bytes(), name.as_bytes())
}

fn glob_match_bytes(pat: &[u8], name: &[u8]) -> bool {
    match (pat.first(), name.first()) {
        // Pattern vide : match seulement si le nom est aussi vide
        (None, None) => true,
        (None, Some(_)) => false,

        // * : match zéro ou plusieurs caractères
        (Some(b'*'), _) => {
            let rest_pat = &pat[1..];
            // Essayer de matcher rest_pat avec toutes les positions de name
            (0..=name.len()).any(|i| glob_match_bytes(rest_pat, &name[i..]))
        }

        // Fin du nom mais pattern non vide
        (Some(_), None) => {
            // Seul un pattern tout-étoiles peut matcher la fin
            pat.iter().all(|&b| b == b'*')
        }

        // ? : match exactement un caractère
        (Some(b'?'), Some(_)) => {
            glob_match_bytes(&pat[1..], &name[1..])
        }

        // [ : classe de caractères
        (Some(b'['), Some(&nc)) => {
            if let Some((matched, rest_pat)) = match_bracket(&pat[1..], nc) {
                if matched {
                    glob_match_bytes(rest_pat, &name[1..])
                } else {
                    false
                }
            } else {
                // '[' non fermé → traiter comme littéral
                nc == b'[' && glob_match_bytes(&pat[1..], &name[1..])
            }
        }

        // Échappement backslash
        (Some(b'\\'), Some(&nc)) if pat.len() >= 2 => {
            pat[1] == nc && glob_match_bytes(&pat[2..], &name[1..])
        }

        // Caractère littéral
        (Some(&pc), Some(&nc)) => {
            pc == nc && glob_match_bytes(&pat[1..], &name[1..])
        }
    }
}

/// Parse une classe de caractères `[...]`.
/// Retourne (matched, rest_of_pattern_after_]) ou None si non fermé.
fn match_bracket(pat: &[u8], c: u8) -> Option<(bool, &[u8])> {
    let mut i = 0;
    let negate = pat.first() == Some(&b'!') || pat.first() == Some(&b'^');
    if negate { i += 1; }

    let mut matched = false;
    let mut prev: Option<u8> = None;

    while i < pat.len() {
        match pat[i] {
            b']' if i > (if negate { 1 } else { 0 }) => {
                // Fermeture de la classe
                return Some((matched != negate, &pat[i + 1..]));
            }
            b'-' if prev.is_some() && i + 1 < pat.len() && pat[i + 1] != b']' => {
                // Plage a-z
                let lo = prev.unwrap();
                let hi = pat[i + 1];
                if c >= lo && c <= hi {
                    matched = true;
                }
                i += 2;
                prev = None;
            }
            b => {
                if b == c { matched = true; }
                prev = Some(b);
                i += 1;
            }
        }
    }
    // Crochet non fermé
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_star() {
        assert!(glob_match("*.rs", "main.rs"));
        assert!(glob_match("*.rs", "lib.rs"));
        assert!(!glob_match("*.rs", "main.txt"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("*", ""));
    }

    #[test]
    fn test_glob_question() {
        assert!(glob_match("?.rs", "a.rs"));
        assert!(!glob_match("?.rs", "ab.rs"));
        assert!(glob_match("file?.txt", "file1.txt"));
    }

    #[test]
    fn test_glob_bracket() {
        assert!(glob_match("[abc].txt", "a.txt"));
        assert!(glob_match("[abc].txt", "b.txt"));
        assert!(!glob_match("[abc].txt", "d.txt"));
    }

    #[test]
    fn test_glob_bracket_range() {
        assert!(glob_match("[0-9].txt", "5.txt"));
        assert!(!glob_match("[0-9].txt", "a.txt"));
        assert!(glob_match("[a-z]argo.toml", "Cargo.toml") == false);
        assert!(glob_match("[Cc]argo.toml", "Cargo.toml"));
    }

    #[test]
    fn test_glob_bracket_negate() {
        assert!(!glob_match("[!abc].txt", "a.txt"));
        assert!(glob_match("[!abc].txt", "d.txt"));
    }

    #[test]
    fn test_glob_literal() {
        assert!(glob_match("Cargo.toml", "Cargo.toml"));
        assert!(!glob_match("Cargo.toml", "cargo.toml"));
    }

    #[test]
    fn test_glob_prefix_star() {
        assert!(glob_match("src/*.rs", "src/main.rs"));
        assert!(!glob_match("src/*.rs", "src/main.txt"));
    }

    #[test]
    fn test_needs_expansion() {
        assert!(needs_expansion("*.rs"));
        assert!(needs_expansion("file?.txt"));
        assert!(needs_expansion("[abc]"));
        assert!(!needs_expansion("Cargo.toml"));
        assert!(!needs_expansion("--flag"));
    }

    #[test]
    fn test_split_dir_pattern() {
        let (d, f) = split_dir_pattern("src/*.rs");
        assert_eq!(d, "src");
        assert_eq!(f, "*.rs");

        let (d, f) = split_dir_pattern("*.toml");
        assert_eq!(d, "");
        assert_eq!(f, "*.toml");
    }
}
