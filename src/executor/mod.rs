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
