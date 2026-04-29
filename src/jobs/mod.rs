// Module gestion des jobs (jobs) pour le shell RustShell
//
// Tâches à implémenter :
// - Définir une structure pour représenter un job (PID, état, commande)
// - Maintenir une liste des jobs en cours (foreground et background)
// - Implémenter les commandes built-in : fg, bg, jobs
// - Gérer les signaux (SIGCHLD) pour mettre à jour l'état des jobs
// - Permettre de passer un job en arrière-plan (&) ou de le ramener en avant-plan


use std::collections::HashMap;
use std::fmt;

#[cfg(unix)]
use std::os::unix::process::CommandExt;

#[derive(Debug, Clone, PartialEq)]
pub enum JobState {
    Running,
    Stopped,
    Done(i32),
}

impl fmt::Display for JobState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            JobState::Running => write!(f, "Running"),
            JobState::Stopped => write!(f, "Stopped"),
            JobState::Done => write!(f, "Done"),
        }
    }
}

#[derive(Debug, Clone)]
pub struct Job {
    pub id: usize,
    pub pid: u32,
    pub state: JobState,
    pub command: String,
    pub background: bool,
}

impl Job {
    pub fn new(id: usize, pid: u32, command: String, background: bool) -> Self {
        Job {
            id,
            pid,
            state: JobState::Running,
            command,
            background,
        }
    }
}

impl fmt::Display for Job {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let bg_indicator = if self.background { "&" } else { "" };
        write!(
            f,
            "[{}]\t{}\t\t{} {}",
            self.id, self.state, self.command, bg_indicator
        )
    }
}

pub struct JobList {
    jobs: HashMap<usize, Job>,
    next_id: usize,
}

impl JobList {
    pub fn new() -> Self {
        JobList {
            jobs: HashMap::new(),
            next_id: 1,
        }
    }

    pub fn add_job(&mut self, pid: u32, command: String, background: bool) -> usize {
        let id = self.next_id;
        let job = Job::new(id, pid, command, background);
        self.jobs.insert(id, job);
        self.next_id += 1;
        id
    }

    pub fn remove_job(&mut self, id: usize) -> Option<Job> {
        self.jobs.remove(&id)
    }

    pub fn get_job(&self, id: usize) -> Option<&Job> {
        self.jobs.get(&id)
    }

    pub fn get_job_mut(&mut self, id: usize) -> Option<&mut Job> {
        self.jobs.get_mut(&id)
    }

    pub fn get_last_job(&self) -> Option<&Job> {
        self.jobs.values().max_by_key(|j| j.id)
    }

    pub fn update_state(&mut self, id: usize, new_state: JobState) {
        if let Some(job) = self.jobs.get_mut(&id) {
            job.state = new_state;
        }
    }

    pub fn find_by_pid(&self, pid: u32) -> Option<&Job> {
        self.jobs.values().find(|j| j.pid == pid)
    }

    pub fn find_by_pid_mut(&mut self, pid: u32) -> Option<&mut Job> {
        self.jobs.values_mut().find(|j| j.pid == pid)
    }

    pub fn cleanup_done(&mut self) {
        let done_ids: Vec<usize> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.state == JobState::Done)
            .map(|(id, _)| *id)
            .collect();

        for id in done_ids {
            if let Some(job) = self.jobs.remove(&id) {
                println!("[{}]\tDone\t\t{}", job.id, job.command);
            }
        }
    }

    // commande built-in : jobs
    pub fn builtin_jobs(&self) {
        let mut sorted: Vec<&Job> = self.jobs.values().collect();
        sorted.sort_by_key(|j| j.id);

        for job in sorted {
            println!("{}", job);
        }
    }

    // commande built-in : fg [job_id]
    #[cfg(unix)]
    pub fn builtin_fg(&mut self, job_id: Option<usize>) {
        let id = match job_id {
            Some(id) => id,
            None => {
                match self.get_last_job() {
                    Some(job) => job.id,
                    None => {
                        eprintln!("fg: pas de job en cours");
                        return;
                    }
                }
            }
        };

        let job = match self.get_job_mut(id) {
            Some(j) => j,
            None => {
                eprintln!("fg: {}: job introuvable", id);
                return;
            }
        };

        if job.state == JobState::Done {
            eprintln!("fg: job {} est déjà terminé", id);
            return;
        }

        println!("{}", job.command);
        job.background = false;
        job.state = JobState::Running;

        let pid = job.pid;

        unsafe {
            // envoyer SIGCONT au processus pour le reprendre
            libc::kill(pid as i32, libc::SIGCONT);
            // mettre le processus en avant-plan dans le terminal
            libc::tcsetpgrp(0, pid as i32);
        }

        // attendre que le processus se termine ou soit stoppé
        let mut status: i32 = 0;
        unsafe {
            libc::waitpid(pid as i32, &mut status, libc::WUNTRACED);
        }

        // remettre le shell en avant-plan
        unsafe {
            let shell_pgid = libc::getpgrp();
            libc::tcsetpgrp(0, shell_pgid);
        }

        if is_stopped(status) {
            self.update_state(id, JobState::Stopped);
            if let Some(job) = self.get_job(id) {
                println!("\n[{}]\tStopped\t\t{}", job.id, job.command);
            }
        } else {
            self.update_state(id, JobState::Done);
            self.cleanup_done();
        }
    }

    #[cfg(not(unix))]
    pub fn builtin_fg(&mut self, job_id: Option<usize>) {
        let id = match job_id {
            Some(id) => id,
            None => {
                match self.get_last_job() {
                    Some(job) => job.id,
                    None => {
                        eprintln!("fg: pas de job en cours");
                        return;
                    }
                }
            }
        };

        if let Some(job) = self.get_job_mut(id) {
            println!("{}", job.command);
            job.background = false;
            job.state = JobState::Running;
        } else {
            eprintln!("fg: {}: job introuvable", id);
        }
    }

    // commande built-in : bg [job_id]
    #[cfg(unix)]
    pub fn builtin_bg(&mut self, job_id: Option<usize>) {
        let id = match job_id {
            Some(id) => id,
            None => {
                match self.get_last_stopped_job() {
                    Some(job) => job.id,
                    None => {
                        eprintln!("bg: pas de job stoppé");
                        return;
                    }
                }
            }
        };

        let job = match self.get_job_mut(id) {
            Some(j) => j,
            None => {
                eprintln!("bg: {}: job introuvable", id);
                return;
            }
        };

        if job.state != JobState::Stopped {
            eprintln!("bg: le job {} n'est pas stoppé", id);
            return;
        }

        job.state = JobState::Running;
        job.background = true;
        let pid = job.pid;
        let cmd = job.command.clone();

        println!("[{}]\t{} &", id, cmd);

        unsafe {
            libc::kill(pid as i32, libc::SIGCONT);
        }
    }

    #[cfg(not(unix))]
    pub fn builtin_bg(&mut self, job_id: Option<usize>) {
        let id = match job_id {
            Some(id) => id,
            None => {
                match self.get_last_stopped_job() {
                    Some(job) => job.id,
                    None => {
                        eprintln!("bg: pas de job stoppé");
                        return;
                    }
                }
            }
        };

        if let Some(job) = self.get_job_mut(id) {
            if job.state != JobState::Stopped {
                eprintln!("bg: le job {} n'est pas stoppé", id);
                return;
            }
            job.state = JobState::Running;
            job.background = true;
            println!("[{}]\t{} &", id, job.command);
        } else {
            eprintln!("bg: {}: job introuvable", id);
        }
    }

    fn get_last_stopped_job(&self) -> Option<&Job> {
        self.jobs
            .values()
            .filter(|j| j.state == JobState::Stopped)
            .max_by_key(|j| j.id)
    }

    // mettre à jour les jobs en vérifiant l'état des processus (SIGCHLD)
    #[cfg(unix)]
    pub fn update_jobs(&mut self) {
        let pids: Vec<(usize, u32)> = self
            .jobs
            .iter()
            .filter(|(_, j)| j.state == JobState::Running && j.background)
            .map(|(id, j)| (*id, j.pid))
            .collect();

        for (id, pid) in pids {
            let mut status: i32 = 0;
            let result = unsafe { libc::waitpid(pid as i32, &mut status, libc::WNOHANG | libc::WUNTRACED) };

            if result > 0 {
                if is_exited(status) || is_signaled(status) {
                    self.update_state(id, JobState::Done);
                } else if is_stopped(status) {
                    self.update_state(id, JobState::Stopped);
                }
            }
        }
    }

    #[cfg(not(unix))]
    pub fn update_jobs(&mut self) {
        // pas de support SIGCHLD sous Windows
    }
}

// lancer une commande en arrière-plan
#[cfg(unix)]
pub fn launch_background(cmd: &str, args: &[&str]) -> Option<u32> {
    use std::process::{Command, Stdio};

    let child = unsafe {
        Command::new(cmd)
            .args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .pre_exec(|| {
                // créer un nouveau groupe de processus
                libc::setpgid(0, 0);
                Ok(())
            })
            .spawn()
    };

    match child {
        Ok(child) => Some(child.id()),
        Err(e) => {
            eprintln!("rustshell: {}: {}", cmd, e);
            None
        }
    }
}

#[cfg(not(unix))]
pub fn launch_background(cmd: &str, args: &[&str]) -> Option<u32> {
    use std::process::{Command, Stdio};

    let child = Command::new(cmd)
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .spawn();

    match child {
        Ok(child) => Some(child.id()),
        Err(e) => {
            eprintln!("rustshell: {}: {}", cmd, e);
            None
        }
    }
}

// installer le handler pour SIGCHLD
#[cfg(unix)]
pub fn setup_sigchld_handler() {
    unsafe {
        libc::signal(libc::SIGCHLD, libc::SIG_DFL);
    }
}

#[cfg(not(unix))]
pub fn setup_sigchld_handler() {
    // pas de SIGCHLD sous Windows
}

// fonctions utilitaires pour analyser le status waitpid
#[cfg(unix)]
fn is_exited(status: i32) -> bool {
    unsafe { libc::WIFEXITED(status) }
}

#[cfg(unix)]
fn is_signaled(status: i32) -> bool {
    unsafe { libc::WIFSIGNALED(status) }
}

#[cfg(unix)]
fn is_stopped(status: i32) -> bool {
    unsafe { libc::WIFSTOPPED(status) }
}

// parser l'argument de fg/bg pour extraire le job id
pub fn parse_job_id(arg: Option<&str>) -> Option<usize> {
    match arg {
        None => None,
        Some(s) => {
            let s = s.trim_start_matches('%');
            s.parse::<usize>().ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_job_creation() {
        let job = Job::new(1, 1234, "sleep 100".to_string(), true);
        assert_eq!(job.id, 1);
        assert_eq!(job.pid, 1234);
        assert_eq!(job.state, JobState::Running);
        assert_eq!(job.command, "sleep 100");
        assert!(job.background);
    }

    #[test]
    fn test_joblist_add_and_get() {
        let mut list = JobList::new();
        let id = list.add_job(1000, "ls -la".to_string(), false);
        assert_eq!(id, 1);

        let job = list.get_job(id).unwrap();
        assert_eq!(job.pid, 1000);
        assert_eq!(job.command, "ls -la");
    }

    #[test]
    fn test_joblist_multiple_jobs() {
        let mut list = JobList::new();
        let id1 = list.add_job(100, "cmd1".to_string(), false);
        let id2 = list.add_job(200, "cmd2".to_string(), true);
        let id3 = list.add_job(300, "cmd3".to_string(), false);

        assert_eq!(id1, 1);
        assert_eq!(id2, 2);
        assert_eq!(id3, 3);

        assert_eq!(list.get_job(2).unwrap().pid, 200);
    }

    #[test]
    fn test_joblist_remove() {
        let mut list = JobList::new();
        list.add_job(100, "echo hello".to_string(), false);
        let removed = list.remove_job(1);
        assert!(removed.is_some());
        assert!(list.get_job(1).is_none());
    }

    #[test]
    fn test_update_state() {
        let mut list = JobList::new();
        let id = list.add_job(100, "sleep 10".to_string(), true);
        list.update_state(id, JobState::Stopped);
        assert_eq!(list.get_job(id).unwrap().state, JobState::Stopped);
    }

    #[test]
    fn test_find_by_pid() {
        let mut list = JobList::new();
        list.add_job(111, "cmd_a".to_string(), false);
        list.add_job(222, "cmd_b".to_string(), true);

        let found = list.find_by_pid(222).unwrap();
        assert_eq!(found.command, "cmd_b");

        assert!(list.find_by_pid(999).is_none());
    }

    #[test]
    fn test_get_last_job() {
        let mut list = JobList::new();
        list.add_job(100, "first".to_string(), false);
        list.add_job(200, "second".to_string(), true);
        list.add_job(300, "third".to_string(), false);

        let last = list.get_last_job().unwrap();
        assert_eq!(last.command, "third");
    }

    #[test]
    fn test_parse_job_id() {
        assert_eq!(parse_job_id(None), None);
        assert_eq!(parse_job_id(Some("3")), Some(3));
        assert_eq!(parse_job_id(Some("%2")), Some(2));
        assert_eq!(parse_job_id(Some("abc")), None);
    }

    #[test]
    fn test_job_display() {
        let job = Job::new(1, 1234, "sleep 100".to_string(), true);
        let display = format!("{}", job);
        assert!(display.contains("[1]"));
        assert!(display.contains("Running"));
        assert!(display.contains("sleep 100"));
        assert!(display.contains("&"));
    }

    #[test]
    fn test_state_display() {
        assert_eq!(format!("{}", JobState::Running), "Running");
        assert_eq!(format!("{}", JobState::Stopped), "Stopped");
        assert_eq!(format!("{}", JobState::Done), "Done");
    }

    #[test]
    fn test_cleanup_done() {
        let mut list = JobList::new();
        list.add_job(100, "cmd1".to_string(), true);
        list.add_job(200, "cmd2".to_string(), true);
        list.update_state(1, JobState::Done);

        list.cleanup_done();

        assert!(list.get_job(1).is_none());
        assert!(list.get_job(2).is_some());
    }
}
