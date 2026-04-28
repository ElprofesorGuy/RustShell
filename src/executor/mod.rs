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
