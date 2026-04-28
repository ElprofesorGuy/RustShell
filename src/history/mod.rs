// Module historique des commandes (history) pour le shell RustShell
//
// Tâches à implémenter :
// - Stocker l'historique des commandes exécutées dans une liste ou un fichier
// - Implémenter la commande built-in 'history' pour afficher l'historique
// - Permettre la navigation dans l'historique avec les flèches haut/bas (nécessite gestion de l'input)
// - Sauvegarder et charger l'historique depuis un fichier (~/.rustshell_history)
// - Gérer la taille maximale de l'historique


use std::fs::{self, OpenOptions};
use std::io::{self, Write, BufRead};
use std::path::PathBuf;

/// Taille maximale de l'historique en mémoire et sur disque.
const MAX_HISTORY: usize = 1000;

/// Gestionnaire d'historique.
pub struct History {
    /// Entrées de l'historique (du plus ancien au plus récent)
    entries: Vec<String>,
    /// Chemin vers le fichier d'historique
    path: PathBuf,
    /// Index de navigation (None = pas en navigation)
    nav_index: Option<usize>,
    /// Sauvegarde de la ligne courante pendant la navigation
    saved_line: String,
}

impl History {
    /// Crée un nouveau gestionnaire et charge l'historique depuis le disque.
    pub fn new() -> Self {
        let path = history_file_path();
        let entries = load_from_disk(&path);
        History {
            entries,
            path,
            nav_index: None,
            saved_line: String::new(),
        }
    }

    /// Ajoute une entrée à l'historique (ignore les doublons consécutifs et les lignes vides).
    pub fn add(&mut self, line: &str) {
        let line = line.trim().to_string();
        if line.is_empty() {
            return;
        }
        // Ne pas dupliquer la dernière entrée
        if self.entries.last().map(|s| s.as_str()) == Some(&line) {
            return;
        }
        self.entries.push(line.clone());
        // Tronquer si nécessaire
        if self.entries.len() > MAX_HISTORY {
            self.entries.remove(0);
        }
        // Sauvegarder immédiatement (append)
        self.append_to_disk(&line);
        // Réinitialiser la navigation
        self.nav_index = None;
        self.saved_line.clear();
    }

    /// Navigation vers le haut (entrée précédente). Retourne la ligne à afficher.
    pub fn prev(&mut self, current_line: &str) -> Option<&str> {
        if self.entries.is_empty() {
            return None;
        }
        match self.nav_index {
            None => {
                // Commencer la navigation : sauvegarder la ligne courante
                self.saved_line = current_line.to_string();
                self.nav_index = Some(self.entries.len() - 1);
            }
            Some(0) => {
                // Déjà au plus ancien, ne pas bouger
                return self.entries.first().map(|s| s.as_str());
            }
            Some(i) => {
                self.nav_index = Some(i - 1);
            }
        }
        self.nav_index.and_then(|i| self.entries.get(i)).map(|s| s.as_str())
    }

    /// Navigation vers le bas (entrée suivante). Retourne la ligne à afficher,
    /// ou None si on est retourné à la ligne courante.
    pub fn next(&mut self) -> Option<&str> {
        match self.nav_index {
            None => None,
            Some(i) if i + 1 >= self.entries.len() => {
                // Retour à la ligne sauvegardée
                self.nav_index = None;
                Some(&self.saved_line)
            }
            Some(i) => {
                self.nav_index = Some(i + 1);
                self.entries.get(i + 1).map(|s| s.as_str())
            }
        }
    }

    /// Réinitialise l'index de navigation (appelé quand l'utilisateur tape).
    pub fn reset_nav(&mut self) {
        self.nav_index = None;
    }

    /// Recherche dans l'historique (pour Ctrl+R, simplifié).
    pub fn search(&self, query: &str) -> Option<&str> {
        self.entries.iter().rev()
            .find(|e| e.contains(query))
            .map(|s| s.as_str())
    }

    /// Nombre d'entrées dans l'historique.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Commande builtin `history` : afficher l'historique.
    pub fn print(&self, n: Option<usize>) {
        let start = n.map(|n| self.entries.len().saturating_sub(n)).unwrap_or(0);
        for (i, entry) in self.entries[start..].iter().enumerate() {
            println!("{:5}  {}", start + i + 1, entry);
        }
    }

    /// Sauvegarder tout l'historique sur le disque (réécriture complète).
    pub fn save_all(&self) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .write(true).create(true).truncate(true).open(&self.path)
        {
            for entry in &self.entries {
                let _ = writeln!(file, "{}", entry);
            }
        }
    }

    fn append_to_disk(&self, line: &str) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        if let Ok(mut file) = OpenOptions::new()
            .write(true).create(true).append(true).open(&self.path)
        {
            let _ = writeln!(file, "{}", line);
        }
    }
}

/// Charge l'historique depuis le fichier disque.
fn load_from_disk(path: &PathBuf) -> Vec<String> {
    if let Ok(file) = std::fs::File::open(path) {
        let reader = io::BufReader::new(file);
        let mut entries: Vec<String> = reader.lines()
            .filter_map(|l| l.ok())
            .filter(|l| !l.trim().is_empty())
            .collect();
        // Garder seulement les MAX_HISTORY dernières entrées
        if entries.len() > MAX_HISTORY {
            entries = entries[entries.len() - MAX_HISTORY..].to_vec();
        }
        entries
    } else {
        Vec::new()
    }
}
/// Retourne le chemin vers le fichier d'historique.
fn history_file_path() -> PathBuf {
    dirs_home().join(".rustshell_history")
}

/// Obtient le répertoire home de l'utilisateur.
fn dirs_home() -> PathBuf {
    std::env::var("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
}

// ─── Autocomplétion ──────────────────────────────────────────────────────────

/// Autocomplétion des noms de fichiers et de commandes.
/// Retourne la liste des complétions pour un préfixe donné.
pub fn complete(prefix: &str, complete_commands: bool) -> Vec<String> {
    let mut completions = Vec::new();

    if prefix.contains('/') || prefix.starts_with('.') || prefix.starts_with('~') {
        // Complétion de chemin
        complete_path(prefix, &mut completions);
    } else if complete_commands {
        // Complétion de commande (PATH + builtins)
        complete_command(prefix, &mut completions);
    } else {
        // Complétion de fichier dans le répertoire courant
        complete_path(prefix, &mut completions);
    }

    completions.sort();
    completions.dedup();
    completions
}

fn complete_path(prefix: &str, completions: &mut Vec<String>) {
    // Séparer le répertoire du préfixe de nom
    let (dir, name_prefix) = if let Some(pos) = prefix.rfind('/') {
        (&prefix[..=pos], &prefix[pos + 1..])
    } else {
        ("./", prefix)
    };

    let dir_path = if dir == "./" { std::path::Path::new(".") } else { std::path::Path::new(dir) };

    if let Ok(entries) = fs::read_dir(dir_path) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(name_prefix) {
                let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
                let full = if dir == "./" {
                    if is_dir {
                        format!("{}/", name_str)
                    } else {
                        name_str.to_string()
                    }
                } else {
                    if is_dir {
                        format!("{}{}/", dir, name_str)
                    } else {
                        format!("{}{}", dir, name_str)
                    }
                };
                completions.push(full);
            }
        }
    }
}

fn complete_command(prefix: &str, completions: &mut Vec<String>) {
    // Builtins
    let builtins = ["cd", "pwd", "exit", "export", "unset", "echo", "true", "false", "type", "jobs", "fg", "bg", "history"];
    for b in &builtins {
        if b.starts_with(prefix) {
            completions.push(b.to_string());
        }
    }

    // Commandes dans $PATH
    if let Ok(path_env) = std::env::var("PATH") {
        for dir in path_env.split(':') {
            if let Ok(entries) = fs::read_dir(dir) {
                for entry in entries.flatten() {
                    let name = entry.file_name();
                    let name_str = name.to_string_lossy().to_string();
                    if name_str.starts_with(prefix) {
                        // Vérifier que c'est exécutable
                        if let Ok(meta) = entry.metadata() {
                            #[cfg(unix)]
                            {
                                use std::os::unix::fs::PermissionsExt;
                                if meta.permissions().mode() & 0o111 != 0 {
                                    completions.push(name_str);
                                }
                            }
                            #[cfg(not(unix))]
                            completions.push(name_str);
                        }
                    }
                }
            }
        }
    }
}

/// Trouve le plus long préfixe commun d'une liste de chaînes.
pub fn common_prefix(completions: &[String]) -> String {
    if completions.is_empty() {
        return String::new();
    }
    if completions.len() == 1 {
        return completions[0].clone();
    }

    let first = &completions[0];
    let mut len = first.len();

    for s in &completions[1..] {
        len = first.chars().zip(s.chars())
            .take_while(|(a, b)| a == b)
            .count()
            .min(len);
    }

    first[..len].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_history() {
        let mut h = History {
            entries: Vec::new(),
            path: PathBuf::from("/tmp/test_rustshell_history"),
            nav_index: None,
            saved_line: String::new(),
        };
        h.add("ls -la");
        h.add("pwd");
        assert_eq!(h.len(), 2);
    }

    #[test]
    fn test_no_duplicates() {
        let mut h = History {
            entries: Vec::new(),
            path: PathBuf::from("/tmp/test_rustshell_history"),
            nav_index: None,
            saved_line: String::new(),
        };
        h.add("ls");
        h.add("ls");
        assert_eq!(h.len(), 1);
    }

    #[test]
    fn test_nav_prev_next() {
        let mut h = History {
            entries: vec!["cmd1".into(), "cmd2".into(), "cmd3".into()],
            path: PathBuf::from("/tmp/test_rustshell_history"),
            nav_index: None,
            saved_line: String::new(),
        };
        assert_eq!(h.prev(""), Some("cmd3"));
        assert_eq!(h.prev(""), Some("cmd2"));
        assert_eq!(h.next(), Some("cmd3"));
    }

    #[test]
    fn test_common_prefix() {
        let comps = vec!["foobar".into(), "foobaz".into(), "fooqix".into()];
        assert_eq!(common_prefix(&comps), "foo");
    }

    #[test]
    fn test_common_prefix_single() {
        let comps = vec!["hello".into()];
        assert_eq!(common_prefix(&comps), "hello");
    }
}