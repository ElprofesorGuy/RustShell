/// Module prompt : prompt dynamique avec branche Git, heure, et code de retour coloré.
///
/// Affiche :  ⚡ user@host ~/projet [main] 14:32 $
/// En rouge si la dernière commande a échoué, vert sinon.

use std::path::Path;

// ── Codes couleurs ANSI ───────────────────────────────────────────────────────
pub const RESET:       &str = "\x1b[0m";
pub const BOLD:        &str = "\x1b[1m";
#[allow(dead_code)]
pub const RED:         &str = "\x1b[31m";
pub const GREEN:       &str = "\x1b[32m";
pub const YELLOW:      &str = "\x1b[33m";
pub const BLUE:        &str = "\x1b[34m";
pub const MAGENTA:     &str = "\x1b[35m";
pub const CYAN:        &str = "\x1b[36m";
#[allow(dead_code)]
pub const WHITE:       &str = "\x1b[37m";
pub const BRIGHT_RED:  &str = "\x1b[91m";
pub const BRIGHT_GREEN:&str = "\x1b[92m";

/// Construit le prompt complet.
/// Format : ⚡ user@host ~/cwd [branche] HH:MM $
pub fn build_prompt(
    user: &str,
    host: &str,
    cwd: &str,
    last_exit: i32,
    is_root: bool,
) -> String {
    let branch = get_git_branch();
    let time   = get_time();

    // Couleur de l'indicateur selon le dernier code de retour
    let (indicator_color, indicator) = if is_root {
        (BRIGHT_RED, "#")
    } else if last_exit == 0 {
        (BRIGHT_GREEN, "$")
    } else {
        (BRIGHT_RED, "$")
    };

    // Partie branche Git (absente si pas dans un dépôt)
    let branch_part = match &branch {
        Some(b) => format!(" {}[{}]{}", YELLOW, b, RESET),
        None    => String::new(),
    };

    // Partie heure
    let time_part = format!(" {}{}{}", CYAN, time, RESET);

    // Assemblage du prompt
    // ⚡ user@host ~/cwd [main] 14:32 $
    format!(
        "{}⚡{} {}{}{}{}@{}{}: {}{}{}{}{} {}{}{}  ",
        YELLOW, RESET,
        BOLD, GREEN, user, RESET,
        MAGENTA, host,
        BLUE, cwd, RESET,
        branch_part,
        time_part,
        indicator_color, indicator, RESET,
    )
}

/// Détecte la branche Git courante en lisant `.git/HEAD`.
/// Ne lance aucun processus externe — lecture de fichier pure.
pub fn get_git_branch() -> Option<String> {
    // Remonter l'arborescence pour trouver .git/HEAD
    let cwd = std::env::current_dir().ok()?;
    let mut dir = cwd.as_path();

    loop {
        let head = dir.join(".git").join("HEAD");
        if head.exists() {
            let content = std::fs::read_to_string(&head).ok()?;
            let content = content.trim();

            // Format normal : "ref: refs/heads/main"
            if let Some(branch) = content.strip_prefix("ref: refs/heads/") {
                return Some(branch.to_string());
            }
            // HEAD détaché : afficher le SHA court
            if content.len() >= 7 {
                return Some(format!(":{}", &content[..7]));
            }
            return Some("HEAD".to_string());
        }

        // Remonter d'un niveau
        dir = dir.parent()?;
        // Ne pas dépasser la racine
        if dir == Path::new("/") {
            break;
        }
    }
    None
}

/// Retourne l'heure courante au format HH:MM.
fn get_time() -> String {
    // Lecture de /proc/... trop complexe sans chrono.
    // On utilise libc::time pour rester sans dépendance.
    unsafe {
        let mut t: libc::time_t = 0;
        libc::time(&mut t);
        let tm = libc::localtime(&t);
        if tm.is_null() {
            return "--:--".to_string();
        }
        format!("{:02}:{:02}", (*tm).tm_hour, (*tm).tm_min)
    }
}

/// Raccourcit le chemin courant en remplaçant $HOME par ~.
pub fn shorten_path(path: &str, home: &str) -> String {
    if path.starts_with(home) && !home.is_empty() {
        format!("~{}", &path[home.len()..])
    } else {
        path.to_string()
    }
}

/// Affiche un message d'erreur en rouge sur stderr.
pub fn print_error(msg: &str) {
    eprintln!("{}{}rustshell: {}{}", BOLD, BRIGHT_RED, RESET, msg);
}

/// Affiche un message de succès/info en vert sur stderr.
#[allow(dead_code)]
pub fn print_success(msg: &str) {
    eprintln!("{}{}{}{}", BOLD, BRIGHT_GREEN, msg, RESET);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shorten_path_home() {
        assert_eq!(shorten_path("/home/alice/projects", "/home/alice"), "~/projects");
    }

    #[test]
    fn test_shorten_path_no_home() {
        assert_eq!(shorten_path("/etc/nginx", "/home/alice"), "/etc/nginx");
    }

    #[test]
    fn test_shorten_path_exact_home() {
        assert_eq!(shorten_path("/home/alice", "/home/alice"), "~");
    }

    #[test]
    fn test_get_time_format() {
        let t = get_time();
        assert_eq!(t.len(), 5);
        assert_eq!(t.chars().nth(2), Some(':'));
    }
}
