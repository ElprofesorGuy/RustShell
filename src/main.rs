/// RustShell v2 — Shell POSIX minimaliste en Rust.
/// Améliorations : prompt dynamique Git, globbing, couleurs, Ctrl+R, $(), script démo.

mod lexer;
mod parser;
mod executor;
mod jobs;
mod history;
mod terminal;
mod prompt;
mod glob;

use executor::ExecContext;
use history::History;
use jobs::{builtin_jobs, builtin_fg, builtin_bg, setup_signal_handlers};
use terminal::{readline, ReadLine};
use prompt::{build_prompt, shorten_path, print_error, RESET, BOLD, YELLOW};

fn main() {
    setup_signal_handlers();

    let mut ctx = ExecContext::new();
    let mut history = History::new();

    if is_interactive() {
        print_banner();
    }

    loop {
        ctx.jobs.reap_zombies();
        ctx.jobs.notify_and_clean();

        let prompt_str = make_prompt(&ctx);

        match readline(&prompt_str, &mut history) {
            ReadLine::Line(line) => {
                let line = line.trim().to_string();
                if line.is_empty() { continue; }

                history.add(&line);

                // Commandes spéciales traitées avant le parsing
                if dispatch_special(&line, &mut ctx, &mut history) {
                    continue;
                }

                // Phase 1 : Lexing
                let tokens = match lexer::tokenize(&line) {
                    Ok(t) => t,
                    Err(e) => {
                        print_error(&e.to_string());
                        ctx.last_exit = 2;
                        continue;
                    }
                };

                // Phase 2 : Parsing
                let cmd_list = match parser::parse(&tokens) {
                    Ok(list) => list,
                    Err(e) => {
                        print_error(&e.to_string());
                        ctx.last_exit = 2;
                        continue;
                    }
                };

                // Phase 3 : Expansion glob + exécution
                ctx.last_exit = executor::execute_list(&cmd_list, &mut ctx);
            }

            ReadLine::Eof => {
                if is_interactive() { eprintln!("\nexit"); }
                break;
            }

            ReadLine::Interrupted => {
                ctx.last_exit = 130;
                continue;
            }
        }
    }

    history.save_all();
}

/// Construit le prompt dynamique complet.
fn make_prompt(ctx: &ExecContext) -> String {
    let user = ctx.env.get("USER")
        .or_else(|| ctx.env.get("LOGNAME"))
        .map(|s| s.as_str())
        .unwrap_or("user");

    let host = ctx.env.get("HOSTNAME")
        .map(|s| s.clone())
        .unwrap_or_else(|| {
            std::fs::read_to_string("/etc/hostname")
                .ok()
                .map(|s| s.trim().to_string())
                .unwrap_or_else(|| "localhost".to_string())
        });

    let raw_cwd = std::env::current_dir()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "?".to_string());

    let home = ctx.env.get("HOME").map(|s| s.as_str()).unwrap_or("");
    let cwd = shorten_path(&raw_cwd, home);

    let is_root = unsafe { libc::getuid() == 0 };

    build_prompt(user, &host, &cwd, ctx.last_exit, is_root)
}

/// Gère les commandes spéciales sans passer par le parser complet.
fn dispatch_special(line: &str, ctx: &mut ExecContext, history: &mut History) -> bool {
    let argv: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
    let cmd = argv.first().map(|s| s.as_str()).unwrap_or("");

    match cmd {
        "history" => {
            let n = argv.get(1).and_then(|s| s.parse().ok());
            history.print(n);
            ctx.last_exit = 0;
            true
        }
        "jobs" => {
            builtin_jobs(&mut ctx.jobs);
            ctx.last_exit = 0;
            true
        }
        "fg" => { ctx.last_exit = builtin_fg(&argv, &mut ctx.jobs); true }
        "bg" => { ctx.last_exit = builtin_bg(&argv, &mut ctx.jobs); true }
        "help" => { print_help(); ctx.last_exit = 0; true }
        "clear" => {
            print!("\x1b[2J\x1b[H");
            let _ = std::io::Write::flush(&mut std::io::stdout());
            ctx.last_exit = 0;
            true
        }
        _ => false,
    }
}

fn is_interactive() -> bool {
    unsafe { libc::isatty(libc::STDIN_FILENO) == 1 }
}

fn print_banner() {
    println!();
    println!("{}{}  ____            _   ____  _          _ _  {}", BOLD, YELLOW, RESET);
    println!("{}{} |  _ \\ _   _ ___| |_/ ___|| |__   ___| | | {}", BOLD, YELLOW, RESET);
    println!("{}{} | |_) | | | / __| __\\___ \\| '_ \\ / _ \\ | | {}", BOLD, YELLOW, RESET);
    println!("{}{} |  _ <| |_| \\__ \\ |_ ___) | | | |  __/ | | {}", BOLD, YELLOW, RESET);
    println!("{}{} |_| \\_\\\\__,_|___/\\__|____/|_| |_|\\___|_|_| {}", BOLD, YELLOW, RESET);
    println!();
    println!("  Version {}0.2.0{} | Rust Shell POSIX", BOLD, RESET);
    println!("  Tapez {}help{} pour la liste des commandes, {}exit{} pour quitter.", BOLD, RESET, BOLD, RESET);
    println!("  Ctrl+R = recherche historique | Tab = autocomplétion | ↑↓ = navigation");
    println!();
}

fn print_help() {
    println!(r#"
{}{}RustShell — Aide complète{}
{}══════════════════════════{}

{}Builtins :{} cd, pwd, echo [-n], export [VAR=val], unset VAR,
           type cmd, jobs, fg [%n], bg [%n], history [n],
           clear, help, exit [code]

{}Opérateurs :{}
  cmd | cmd2         Pipe : stdout → stdin
  cmd > fichier      Redirection stdout (troncature)
  cmd >> fichier     Redirection stdout (append)
  cmd < fichier      Redirection stdin
  cmd 2> fichier     Redirection stderr
  cmd &> fichier     Stdout + stderr
  cmd &              Arrière-plan
  cmd1 ; cmd2        Séquence inconditionnelle
  cmd1 && cmd2       ET logique (si cmd1 réussit)
  cmd1 || cmd2       OU logique (si cmd1 échoue)

{}Expansions :{}
  $VAR / ${{VAR}}      Variable d'environnement
  $?                 Code de retour précédent
  $(cmd)             Substitution de commande
  *.rs / ?.toml      Glob : expansion de fichiers
  [abc] / [a-z]      Classe de caractères

{}Raccourcis clavier :{}
  ↑ / ↓             Navigation historique
  ← / →             Déplacer le curseur
  Ctrl+R             Recherche inverse dans l'historique
  Ctrl+A / Ctrl+E    Début / fin de ligne
  Ctrl+K             Supprimer jusqu'à la fin
  Ctrl+U             Supprimer jusqu'au début
  Ctrl+W             Supprimer le mot précédent
  Tab                Autocomplétion fichiers/commandes
  Ctrl+C             Annuler la ligne
  Ctrl+D             EOF / quitter
"#,
    BOLD, YELLOW, RESET,
    BOLD, RESET,
    BOLD, RESET,
    BOLD, RESET,
    BOLD, RESET,
    BOLD, RESET,
    );
}
