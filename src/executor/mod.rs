//! Module executor : exécution des commandes simples et builtins.
//!
//! Résout les chemins via $PATH, fork/exec via std::process::Command,
//! gère les codes de retour, et implémente cd, pwd, exit, export.
//!
//! PARTIE DE JACQUES : STRUCTURE ET EXECUTION CORE
//! Tâches : Imports, ExecContext, execute_list, execute_pipeline, pipelines multi, build_process, redirections, et resolve_path.
//! 
//! 


use std::collections::HashMap;
use std::env;
use std::fs::OpenOptions;
use std::os::unix::io::FromRawFd;
use std::process::{self, Stdio};

use crate::jobs::JobTable;
use crate::glob::expand_args;
use crate::parser::{Command, CommandList, Connector, Pipeline, RedirectKind, RedirectTarget};

/// Contexte d'exécution partagé entre toutes les commandes.
pub struct ExecContext {
    /// Variables d'environnement du shell (exportées aux processus enfants)
    pub env: HashMap<String, String>,
    /// Dernier code de retour
    pub last_exit: i32,
    /// Table des jobs actifs
    pub jobs: JobTable,
}

impl ExecContext {
    pub fn new() -> Self {
        // Hériter des variables d'environnement du processus parent
        let env: HashMap<String, String> = env::vars().collect();
        ExecContext {
            env,
            last_exit: 0,
            jobs: JobTable::new(),
        }
    }
}




/// Point d'entrée : exécute une CommandList complète.
pub fn execute_list(list: &CommandList, ctx: &mut ExecContext) -> i32 {
    let mut last_exit = ctx.last_exit;

    for item in &list.items {
        // Évaluer la condition d'exécution selon le connecteur précédent
        let should_run = match item.connector {
            Connector::None | Connector::Semicolon => true,
            Connector::And => last_exit == 0,
            Connector::Or => last_exit != 0,
        };

        if should_run {
            last_exit = execute_pipeline(&item.pipeline, ctx);
            ctx.last_exit = last_exit;
        }
    }

    last_exit
}

/// Exécute un pipeline (une ou plusieurs commandes reliées par pipes).
pub fn execute_pipeline(pipeline: &Pipeline, ctx: &mut ExecContext) -> i32 {
    let commands = &pipeline.commands;

    if commands.is_empty() {
        return 0;
    }

    // Cas simple : une seule commande (pas de pipe)
    if commands.len() == 1 {
        return execute_command(&commands[0], ctx, None, None, pipeline.background);
    }

    // Pipeline multi-commandes : créer les pipes
    execute_pipeline_multi(commands, ctx, pipeline.background)
}


/// Gère un pipeline de N commandes avec N-1 pipes.
fn execute_pipeline_multi(commands: &[Command], ctx: &mut ExecContext, background: bool) -> i32 {
    use libc::{close, pipe};

    let n = commands.len();
    let mut pipes: Vec<(i32, i32)> = Vec::with_capacity(n - 1);

    // Créer tous les pipes nécessaires
    for _ in 0..n - 1 {
        let mut fds = [0i32; 2];
        unsafe {
            if pipe(fds.as_mut_ptr()) != 0 {
                eprintln!("rustshell: impossible de créer un pipe");
                return 1;
            }
        }
        pipes.push((fds[0], fds[1]));
    }

    let mut children: Vec<process::Child> = Vec::new();

    for (i, cmd) in commands.iter().enumerate() {
        // Si c'est un builtin dans un pipeline, on doit le gérer différemment
        let stdin_fd: Option<i32> = if i == 0 { None } else { Some(pipes[i - 1].0) };
        let stdout_fd: Option<i32> = if i == n - 1 { None } else { Some(pipes[i].1) };

        // Construire la commande avec redirections de pipes
        match build_process(cmd, ctx, stdin_fd, stdout_fd) {
            Ok(Some(mut child)) => {
                // Fermer les fds dont l'enfant a hérité dans le parent
                unsafe {
                    if let Some(fd) = stdin_fd { close(fd); }
                    if let Some(fd) = stdout_fd { close(fd); }
                }
                children.push(child);
            }
            Ok(None) => {}
            Err(e) => {
                eprintln!("{}", e);
                for (r, w) in &pipes { unsafe { close(*r); close(*w); } }
                return 127;
            }
        }
    }

    // Fermer tous les fds de pipes dans le parent
    for (r, w) in &pipes { unsafe { close(*r); close(*w); } }

    let mut last_exit = 0;
    for (i, mut child) in children.into_iter().enumerate() {
        if background && i == 0 {
            let pid = child.id();
            ctx.jobs.add(pid, "pipeline");
            last_exit = 0;
        } else {
            last_exit = child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1);
        }
    }
    last_exit
}



pub fn execute_command(
    cmd: &Command,
    ctx: &mut ExecContext,
    stdin_override: Option<i32>,
    stdout_override: Option<i32>,
    background: bool,
) -> i32 {
    if cmd.argv.is_empty() { return 0; }

    let expanded_argv: Vec<String> = cmd.argv.iter().map(|arg| expand_vars(arg, ctx)).collect();
    let expanded_argv = expand_args(&expanded_argv);

    if let Some(exit_code) = try_builtin(&expanded_argv, ctx) { return exit_code; }

    match build_process_with_argv(cmd, &expanded_argv, ctx, stdin_override, stdout_override) {
        Ok(Some(mut child)) => {
            if background {
                let pid = child.id();
                ctx.jobs.add(pid, expanded_argv.join(" ").as_str());
                println!("[{}] {}", ctx.jobs.len(), pid);
                0
            } else {
                child.wait().map(|s| s.code().unwrap_or(1)).unwrap_or(1)
            }
        }
        Ok(None) => ctx.last_exit,
        Err(e) => { eprintln!("{}", e); 127 }
    }
}

fn build_process(
    cmd: &Command,
    ctx: &mut ExecContext,
    stdin_fd: Option<i32>,
    stdout_fd: Option<i32>,
) -> Result<Option<process::Child>, String> {
    let expanded_argv: Vec<String> = cmd.argv.iter().map(|arg| expand_vars(arg, ctx)).collect();
    build_process_with_argv(cmd, &expanded_argv, ctx, stdin_fd, stdout_fd)
}

fn build_process_with_argv(
    cmd: &Command,
    argv: &[String],
    ctx: &mut ExecContext,
    stdin_fd: Option<i32>,
    stdout_fd: Option<i32>,
) -> Result<Option<process::Child>, String> {
    if argv.is_empty() { return Err("rustshell: commande vide".into()); }
    let name = &argv[0];
    let path = resolve_path(name, ctx).ok_or_else(|| format!("rustshell: {}: commande introuvable", name))?;
    let mut proc = process::Command::new(&path);
    proc.args(&argv[1..]);
    proc.env_clear();
    for (k, v) in &ctx.env { proc.env(k, v); }

    if let Some(fd) = stdin_fd {
        let f = unsafe { std::fs::File::from_raw_fd(fd) };
        proc.stdin(Stdio::from(f));
    } else { apply_stdin_redirect(cmd, &mut proc)?; }

    if let Some(fd) = stdout_fd {
        let f = unsafe { std::fs::File::from_raw_fd(fd) };
        proc.stdout(Stdio::from(f));
    } else { apply_stdout_redirect(cmd, &mut proc)?; }

    apply_stderr_redirect(cmd, &mut proc)?;
    let child = proc.spawn().map_err(|e| format!("rustshell: {}: {}", name, e))?;
    Ok(Some(child))
}
