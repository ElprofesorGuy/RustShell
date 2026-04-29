# RustShell 🦀 v0.2

> Un shell POSIX minimaliste écrit de zéro en Rust — avec prompt dynamique Git, globbing, substitution de commande `$()`, et recherche historique Ctrl+R.

RustShell est un projet pédagogique de programmation système : lexer, parser, exécuteur de commandes, pipes Unix, redirections de flux, gestion des jobs et signaux, historique persistant, autocomplétion — tout implémenté à la main, sans bibliothèque shell tierce.

```
user@host:~$ echo "Bonjour depuis RustShell !"
Bonjour depuis RustShell !
user@host:~$ ls src/ | grep mod | wc -l
5
user@host:~$ sleep 5 &
[1] 12345
user@host:~$ jobs
[1]    12345  En cours  sleep 5
```

---

## Table des matières

1. [Fonctionnalités](#fonctionnalités)
2. [Architecture](#architecture)
3. [Prérequis](#prérequis)
4. [Installation](#installation)
5. [Utilisation](#utilisation)
6. [Référence des commandes](#référence-des-commandes)
7. [Structure du code](#structure-du-code)
8. [Tests](#tests)
9. [Compilation en release](#compilation-en-release)
10. [Feuille de route](#feuille-de-route)
11. [Ressources](#ressources)

---

## Fonctionnalités

### Partie 1 — Lexer & Parser
- Tokenisation complète : mots, opérateurs `|`, `>`, `<`, `>>`, `&`, `;`, `&&`, `||`
- Guillemets simples `'...'` (littéral strict) et doubles `"..."` (avec expansion)
- Échappement backslash `\`
- Commentaires `#`
- Représentation structurée `CommandList → Pipeline → Command`

### Partie 2 — Exécuteur de commandes
- Résolution du chemin via `$PATH`
- Fork/exec via `std::process::Command`
- Gestion des codes de retour (`$?`)
- Expansion des variables `$VAR`, `${VAR}`, `$?`
- **Builtins :** `cd`, `pwd`, `echo`, `export`, `unset`, `type`, `true`, `false`, `exit`

### Partie 3 — Pipes & Redirections
- Chaînage de commandes via pipes Unix (`libc::pipe`)
- Redirection stdout `>` et `>>`
- Redirection stdin `<`
- Redirection stderr `2>`
- Redirection stdout+stderr `&>`
- Gestion correcte de la fermeture des descripteurs de fichiers

### Partie 4 — Jobs & Signaux
- Exécution en arrière-plan avec `&`
- Table des jobs avec numérotation
- Commandes `jobs`, `fg [%n]`, `bg [%n]`
- Signaux : SIGINT (Ctrl+C) et SIGTSTP (Ctrl+Z) ignorés dans le shell principal
- Récolte des zombies via `waitpid(WNOHANG)`
- Notification asynchrone des jobs terminés

### Partie 5 — Historique & Autocomplétion
- Sauvegarde persistante dans `~/.rustshell_history` (1000 entrées max)
- Navigation ↑/↓ dans l'historique
- Édition de ligne en mode raw terminal (flèches ←/→, Suppr, Origine, Fin)
- Autocomplétion Tab : noms de fichiers et commandes `$PATH`
- Complétion du préfixe commun (comportement bash-like)
- Commande `history [n]`

---

## Architecture

```
rustshell/
├── Cargo.toml
├── README.md
└── src/
    ├── main.rs           # Boucle REPL, prompt coloré, dispatching
    ├── terminal.rs       # Mode raw terminal, readline interactif
    ├── lexer/
    │   └── mod.rs        # Tokenisation : Token, LexError, tokenize()
    ├── parser/
    │   └── mod.rs        # AST : CommandList, Pipeline, Command, parse()
    ├── executor/
    │   └── mod.rs        # Exécution, builtins, expand_vars(), ExecContext
    ├── jobs/
    │   └── mod.rs        # JobTable, Job, JobStatus, fg/bg, signaux
    └── history/
        └── mod.rs        # History, persist, navigation, autocomplétion
```

### Pipeline de traitement d'une commande

```
Saisie utilisateur
       │
       ▼
  lexer::tokenize()          "ls -la | grep .rs > out.txt"
       │                      → [Word("ls"), Word("-la"), Pipe, ...]
       ▼
  parser::parse()             → CommandList {
       │                           Pipeline [
       │                             Command { argv: ["ls","-la"] },
       │                             Command { argv: ["grep",".rs"],
       │                                       redirects: [Stdout→"out.txt"] }
       │                           ]
       │                         }
       ▼
  executor::execute_list()    fork/exec, pipe(2), dup2(2), waitpid(2)
       │
       ▼
  ctx.last_exit               code de retour → $?
```

---

## Prérequis

| Outil | Version minimale | Notes |
|-------|-----------------|-------|
| Rust  | 1.70.0          | `rustup update stable` |
| Cargo | (inclus)         | Gestionnaire de paquets Rust |
| Linux | Kernel 3.x+     | Appels système POSIX requis |
| glibc | 2.17+           | `libc` crate |

> **Note :** RustShell cible exclusivement les systèmes POSIX (Linux, macOS). Windows n'est pas supporté (fork/exec, termios).

---

## Installation

### Depuis les sources (recommandé)

```bash
# 1. Cloner le dépôt
git clone https://github.com/votre-compte/rustshell.git
cd rustshell

# 2. Compiler en mode release
cargo build --release

# 3. Lancer directement
./target/release/rustshell

# 4. (Optionnel) Installer dans $PATH
sudo cp target/release/rustshell /usr/local/bin/rustshell
```

### Vérification de l'installation

```bash
rustshell --version   # pas encore implémenté : rustshell v0.1.0
echo $SHELL           # vérifier depuis l'intérieur du shell
```

### Définir comme shell par défaut (optionnel, avancé)

```bash
# Ajouter rustshell aux shells autorisés
echo /usr/local/bin/rustshell | sudo tee -a /etc/shells

# Changer le shell de login
chsh -s /usr/local/bin/rustshell
```

> ⚠️ Faites une sauvegarde de votre shell courant avant de changer le shell de login.

---

## Utilisation

### Lancement interactif

```bash
./target/release/rustshell
```

```
RustShell v0.1.0 — tapez 'exit' pour quitter, 'help' pour l'aide
user@hostname:~$
```

### Mode non-interactif (scripts)

```bash
# Exécuter un script
rustshell < script.sh

# Pipeline Unix
echo "ls -la" | rustshell
```

### Raccourcis clavier

| Raccourci | Action |
|-----------|--------|
| `↑` / `↓` | Navigation dans l'historique |
| `←` / `→` | Déplacer le curseur dans la ligne |
| `Tab`     | Autocomplétion fichier/commande |
| `Ctrl+C`  | Annuler la ligne courante (exit 130) |
| `Ctrl+D`  | EOF — quitter si ligne vide |
| `Backspace` | Supprimer le caractère avant le curseur |
| `Suppr`   | Supprimer le caractère sous le curseur |
| `Origine` | Aller au début de la ligne |
| `Fin`     | Aller à la fin de la ligne |

---

## Référence des commandes

### Builtins

```bash
cd [répertoire]        # Changer de répertoire ; sans arg → $HOME
pwd                    # Afficher le répertoire courant
echo [-n] [args...]    # Afficher ; -n supprime le saut de ligne
export [VAR=valeur]    # Exporter/définir une variable ; sans arg → liste
unset VAR              # Supprimer une variable d'environnement
type commande          # Afficher si builtin ou chemin de l'exécutable
true                   # Retourner 0
false                  # Retourner 1
exit [code]            # Quitter (code = $? par défaut)
jobs                   # Lister les jobs actifs
fg [%n]                # Ramener le job n (ou le dernier) au premier plan
bg [%n]                # Reprendre le job n en arrière-plan
history [n]            # Afficher les n dernières entrées (toutes si absent)
help                   # Afficher l'aide intégrée
```

### Opérateurs

```bash
# Pipe : chaîner stdout → stdin
ls -la | grep ".rs" | wc -l

# Redirections
echo "hello" > fichier.txt     # stdout → fichier (troncature)
echo "world" >> fichier.txt    # stdout → fichier (append)
sort < liste.txt               # stdin ← fichier
make 2> erreurs.log            # stderr → fichier
make &> tout.log               # stdout + stderr → fichier

# Arrière-plan
sleep 10 &
firefox &

# Séquence inconditionnelle
echo "début" ; make ; echo "fin"

# Opérateurs logiques
mkdir -p build && cd build     # cd seulement si mkdir réussit
ping -c1 host || echo "hors ligne"  # message seulement si ping échoue
```

### Variables d'environnement

```bash
export NOM="Alice"             # Définir et exporter
echo $NOM                      # Expansion simple
echo ${NOM}_suffix             # Expansion avec délimiteurs
echo "Code retour: $?"         # Dernier code de retour
echo $HOME $PATH $PWD          # Variables système
unset NOM                      # Supprimer
```

### Exemples pratiques

```bash
# Compiler et tester en une commande
cargo build && cargo test

# Chercher des fichiers Rust contenant "fn main"
find . -name "*.rs" | xargs grep "fn main"

# Rediriger erreurs et résultats séparément
make 2>erreurs.log >sortie.log

# Job en arrière-plan + notification
sleep 30 &
echo "PID: $!"       # NOTE: $! pas encore implémenté
jobs                 # voir le job

# Historique filtré
history 20           # 20 dernières commandes
```

---

## Structure du code

### `src/lexer/mod.rs` — Tokenisation

Le lexer transforme la chaîne brute en `Vec<Token>`. Il gère :

```rust
pub enum Token {
    Word(String),     // mot, argument, chemin
    Pipe,             // |
    RedirectOut,      // >
    RedirectAppend,   // >>
    RedirectIn,       // <
    RedirectErr,      // 2>
    RedirectErrOut,   // &>
    Background,       // &
    Semicolon,        // ;
    And,              // &&
    Or,               // ||
    Eof,
}
```

Les guillemets (`'` et `"`) sont gérés dans `read_single_quoted()` et `read_double_quoted()`. L'échappement backslash est traité dans `read_word()`.

### `src/parser/mod.rs` — Construction de l'AST

Le parser construit une hiérarchie `CommandList → PipelineItem → Pipeline → Command` :

```rust
pub struct Command {
    pub argv: Vec<String>,      // ["grep", "-r", "fn main"]
    pub redirects: Vec<Redirect>,
    pub background: bool,
}

pub struct Pipeline {
    pub commands: Vec<Command>, // commandes reliées par |
    pub background: bool,
}
```

### `src/executor/mod.rs` — Exécution

`ExecContext` est le contexte partagé qui traverse toute l'exécution :

```rust
pub struct ExecContext {
    pub env: HashMap<String, String>,  // variables d'environnement
    pub last_exit: i32,                // $?
    pub jobs: JobTable,                // processus en arrière-plan
}
```

La résolution de chemin (`resolve_path`) parcourt `$PATH` entrée par entrée. L'expansion de variables (`expand_vars`) traite `$VAR`, `${VAR}`, `$?` en une passe sur les octets.

### `src/jobs/mod.rs` — Gestion des jobs

```rust
pub struct JobTable {
    jobs: HashMap<usize, Job>,
    next_id: usize,
}
```

`reap_zombies()` appelle `waitpid(WNOHANG)` pour collecter les processus terminés sans bloquer. `notify_and_clean()` affiche les notifications `[n]+ Terminé cmd` avant chaque prompt.

### `src/history/mod.rs` — Historique persistant

L'historique est chargé depuis `~/.rustshell_history` au démarrage et mis à jour par append à chaque nouvelle entrée. La navigation `prev()`/`next()` maintient un `nav_index` et sauvegarde la ligne en cours d'édition dans `saved_line`.

### `src/terminal.rs` — Mode raw

```
tcgetattr() → sauvegarder termios
tcsetattr() → désactiver ECHO + ICANON (mode raw)
boucle read_byte() → dispatcher selon le code
tcsetattr() → restaurer termios
```

Les séquences ANSI `ESC [ A/B/C/D` correspondent aux flèches ↑↓←→.

---

## Tests

```bash
# Lancer tous les tests unitaires
cargo test

# Lancer les tests d'un module spécifique
cargo test lexer
cargo test parser
cargo test executor
cargo test history
cargo test jobs

# Avec affichage des prints (utile pour déboguer)
cargo test -- --nocapture

# Tests en mode verbose
cargo test -- --test-threads=1
```

### Couverture des tests (32 tests)

| Module     | Tests | Couverture |
|------------|-------|-----------|
| `lexer`    | 9     | Tokenisation, guillemets, opérateurs, commentaires |
| `parser`   | 6     | Pipelines, redirections, séquences, &&/\|\| |
| `executor` | 5     | Résolution PATH, expand_vars, builtins |
| `jobs`     | 5     | CRUD table, statuts, parse_job_arg |
| `history`  | 5     | Ajout, déduplication, navigation, prefix commun |
| `terminal` | 2     | (intégration manuelle) |

---

## Compilation en release

```bash
# Binaire optimisé, LTO activé, symboles strippés
cargo build --release

# Taille du binaire (typiquement ~500 Ko)
ls -lh target/release/rustshell

# Vérifier les dépendances dynamiques
ldd target/release/rustshell

# Profiler avec perf (optionnel)
cargo build --profile=release-with-debug
perf record ./target/release-with-debug/rustshell
perf report
```

### Options du profil release (`Cargo.toml`)

```toml
[profile.release]
opt-level = 3       # Optimisation maximale
lto = true          # Link-Time Optimization
codegen-units = 1   # Meilleure optimisation inter-modules
strip = true        # Supprimer les symboles de debug
```

---

## Feuille de route

### v0.2 — Améliorations prioritaires
- [ ] `$!` (PID du dernier job en arrière-plan)
- [ ] Expansion de tilde `~user`
- [ ] Globbing `*`, `?`, `[...]`
- [ ] Here-documents `<< EOF`
- [ ] `Ctrl+R` : recherche interactive dans l'historique

### v0.3 — Fonctionnalités avancées
- [ ] Scripts shell (fichiers `.sh`)
- [ ] Structures de contrôle : `if/then/else/fi`, `while/do/done`, `for`
- [ ] Fonctions shell : `fn() { ... }`
- [ ] Substitution de commande `$(cmd)` et `` `cmd` ``
- [ ] Arithmétique `$((expr))`

### v0.4 — Qualité de vie
- [ ] Fichier de configuration `~/.rustshellrc`
- [ ] Alias : `alias ll='ls -la'`
- [ ] Prompt configurable via `$PS1`
- [ ] Complétion contextuelle avancée (options, sous-commandes)
- [ ] Support macOS (termios BSD)

---

## Dépendances

```toml
[dependencies]
libc         = "0.2"   # Appels système POSIX : pipe, fork, waitpid, termios...
signal-hook  = "0.3"   # Gestionnaires de signaux sûrs pour Rust
signal-hook-registry = "1.4"
```

Aucune dépendance shell, readline, ou ncurses. Tout est implémenté from scratch.

---

## Ressources

### Comprendre les shells
- [*The POSIX Shell and Utilities specification*](https://pubs.opengroup.org/onlinepubs/9699919799/) — La référence normative
- [*Writing a Unix Shell*](https://indradhanush.github.io/blog/writing-a-unix-shell-part-1/) — Série de tutoriels en C
- [*Advanced Programming in the UNIX Environment*](https://www.apuebook.com/) — Stevens & Rago

### Rust & Programmation système
- [*The Rustonomicon*](https://doc.rust-lang.org/nomicon/) — Rust unsafe & FFI
- [*The Linux Programming Interface*](https://man7.org/tlpi/) — Kerrisk — référence système Linux
- [`std::process`](https://doc.rust-lang.org/std/process/) — Documentation Rust
- [Crate `libc`](https://docs.rs/libc/) — Bindings POSIX pour Rust

### Pages de manuel essentielles
```bash
man 2 fork      # fork(2) — créer un processus
man 2 execve    # execve(2) — remplacer l'image du processus
man 2 pipe      # pipe(2) — créer un pipe Unix
man 2 dup2      # dup2(2) — dupliquer un descripteur de fichier
man 2 waitpid   # waitpid(2) — attendre un processus enfant
man 2 kill      # kill(2) — envoyer un signal
man 3 tcgetattr # termios(3) — contrôle du terminal
man 7 signal    # signal(7) — liste des signaux POSIX
```

---

## Licence

MIT — voir [LICENSE](LICENSE).

---

## Contribution

Les contributions sont les bienvenues. Quelques règles :

1. `cargo test` doit passer sans erreur
2. `cargo clippy` sans warnings
3. Documenter les fonctions publiques (`///`)
4. Un commit = une fonctionnalité ou un fix

```bash
# Vérifications avant commit
cargo fmt
cargo clippy -- -D warnings
cargo test
```
