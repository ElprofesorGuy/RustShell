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