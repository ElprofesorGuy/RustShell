// Module gestion des jobs (jobs) pour le shell RustShell
//
// Tâches à implémenter :
// - Définir une structure pour représenter un job (PID, état, commande)
// - Maintenir une liste des jobs en cours (foreground et background)
// - Implémenter les commandes built-in : fg, bg, jobs
// - Gérer les signaux (SIGCHLD) pour mettre à jour l'état des jobs
// - Permettre de passer un job en arrière-plan (&) ou de le ramener en avant-plan