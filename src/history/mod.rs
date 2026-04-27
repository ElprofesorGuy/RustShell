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