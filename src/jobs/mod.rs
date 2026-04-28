// Module gestion des jobs (jobs) pour le shell RustShell
//
// Tâches à implémenter :
// - Définir une structure pour représenter un job (PID, état, commande)
// - Maintenir une liste des jobs en cours (foreground et background)
// - Implémenter les commandes built-in : fg, bg, jobs
// - Gérer les signaux (SIGCHLD) pour mettre à jour l'état des jobs
// - Permettre de passer un job en arrière-plan (&) ou de le ramener en avant-plan


/// Module jobs : gestion des processus en arrière-plan et des signaux.
///
/// Maintient une table des jobs actifs, implémente les commandes jobs/fg/bg,
/// et configure les gestionnaires de signaux SIGCHLD, SIGINT, SIGTSTP.

use std::collections::HashMap;

/// État d'un job.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Running,
    Stopped,
    Done(i32),
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Running => write!(f, "En cours"),
            JobStatus::Stopped => write!(f, "Stoppé"),
            JobStatus::Done(code) => write!(f, "Terminé ({})", code),
        }
    }
}

/// Un job : processus en arrière-plan ou stoppé.
#[derive(Debug, Clone)]
pub struct Job {
    /// Numéro de job affiché à l'utilisateur (1-indexed)
    pub id: usize,
    /// PID du processus chef de file du job
    pub pid: u32,
    /// Commande d'origine (pour affichage)
    pub command: String,
    /// État courant
    pub status: JobStatus,
}

impl std::fmt::Display for Job {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {:>7}  {}  {}", self.id, self.pid, self.status, self.command)
    }
}

/// Table des jobs actifs du shell.
pub struct JobTable {
    jobs: HashMap<usize, Job>,
    next_id: usize,
}

impl JobTable {
    pub fn new() -> Self {
        JobTable {
            jobs: HashMap::new(),
            next_id: 1,
        }
    }

    /// Ajouter un nouveau job.
    pub fn add(&mut self, pid: u32, command: &str) -> usize {
        let id = self.next_id;
        self.next_id += 1;
        self.jobs.insert(id, Job {
            id,
            pid,
            command: command.to_string(),
            status: JobStatus::Running,
        });
        id
    }

    /// Retirer un job terminé.
    pub fn remove(&mut self, id: usize) {
        self.jobs.remove(&id);
    }

    /// Marquer un job comme stoppé.
    pub fn set_stopped(&mut self, pid: u32) {
        for job in self.jobs.values_mut() {
            if job.pid == pid {
                job.status = JobStatus::Stopped;
                break;
            }
        }
    }

    /// Marquer un job comme terminé.
    pub fn set_done(&mut self, pid: u32, code: i32) {
        for job in self.jobs.values_mut() {
            if job.pid == pid {
                job.status = JobStatus::Done(code);
                break;
            }
        }
    }

    /// Trouver un job par numéro.
    pub fn get(&self, id: usize) -> Option<&Job> {
        self.jobs.get(&id)
    }

    /// Dernier job (pour fg/bg sans argument).
    pub fn last(&self) -> Option<&Job> {
        self.jobs.values().max_by_key(|j| j.id)
    }

    /// Nombre de jobs.
    pub fn len(&self) -> usize {
        self.jobs.len()
    }

    /// Afficher tous les jobs (commande `jobs`).
    pub fn print_all(&self) {
        let mut jobs: Vec<_> = self.jobs.values().collect();
        jobs.sort_by_key(|j| j.id);
        for job in jobs {
            println!("{}", job);
        }
    }

    /// Vérifier les jobs terminés (via waitpid non-bloquant) et mettre à jour la table.
    pub fn reap_zombies(&mut self) {
        use libc::{waitpid, WNOHANG, WIFEXITED, WEXITSTATUS, WIFSTOPPED};

        let pids: Vec<u32> = self.jobs.values().map(|j| j.pid).collect();
        for pid in pids {
            let mut status = 0i32;
            let ret = unsafe {
                waitpid(pid as libc::pid_t, &mut status, WNOHANG)
            };
            if ret == pid as libc::pid_t {
                let exit_code = if unsafe { WIFEXITED(status) } {
                    unsafe { WEXITSTATUS(status) }
                } else {
                    1
                };
                self.set_done(pid, exit_code);
            }
        }
    }