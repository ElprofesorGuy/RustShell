// Module gestion des pipes (pipes) pour le shell RustShell
//
// Tâches à implémenter :
// - Implémenter une fonction pour créer un pipeline de commandes
// - Utiliser des pipes Unix (std::os::unix::io) pour connecter stdout d'une commande à stdin de la suivante
// - Gérer le fork et l'exécution des processus dans le pipeline
// - Attendre la fin de tous les processus du pipeline et collecter les codes de retour
// - Traiter les erreurs de création de pipes ou d'exécution