// Module gestion des pipes (pipes) pour le shell RustShell
//
// Tâches à implémenter :
// - Implémenter une fonction pour créer un pipeline de commandes
// - Utiliser des pipes Unix (std::os::unix::io) pour connecter stdout d'une commande à stdin de la suivante
// - Gérer le fork et l'exécution des processus dans le pipeline
// - Attendre la fin de tous les processus du pipeline et collecter les codes de retour
// - Traiter les erreurs de création de pipes ou d'exécution
use crate::lexer::Token;

/// Cible d'une redirection de fichier.
#[derive(Debug, Clone)]
pub enum RedirectTarget {
    /// Fichier (troncature)
    File(String),
    /// Fichier (append)
    FileAppend(String),
}

/// Une redirection de flux : source + cible.
#[derive(Debug, Clone)]
pub struct Redirect {
    pub kind: RedirectKind,
    pub target: RedirectTarget,
}

#[derive(Debug, Clone, PartialEq)]
pub enum RedirectKind {
    Stdin,
    Stdout,
    Stderr,
    StdoutStderr,
}

/// Une commande simple avec ses arguments et ses redirections.
#[derive(Debug, Clone)]
pub struct Command {
    /// Nom de la commande + arguments (argv[0..])
    pub argv: Vec<String>,
    /// Redirections associées à cette commande
    pub redirects: Vec<Redirect>,
    /// La commande doit-elle tourner en arrière-plan ?
    pub background: bool,
}

impl Command {
    pub fn new(argv: Vec<String>) -> Self {
        Command {
            argv,
            redirects: Vec::new(),
            background: false,
        }
    }

    pub fn name(&self) -> Option<&str> {
        self.argv.first().map(|s| s.as_str())
    }
}

/// Un pipeline : une ou plusieurs commandes reliées par des pipes.
#[derive(Debug, Clone)]
pub struct Pipeline {
    pub commands: Vec<Command>,
    pub background: bool,
}

/// Une liste de pipelines reliés par ;, &&, ||.
#[derive(Debug, Clone)]
pub struct CommandList {
    pub items: Vec<PipelineItem>,
}

#[derive(Debug, Clone)]
pub struct PipelineItem {
    pub pipeline: Pipeline,
    pub connector: Connector,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Connector {
    /// Fin de liste (dernier élément)
    None,
    /// ;  — exécution séquentielle inconditionnelle
    Semicolon,
    /// && — exécuter le suivant seulement si le précédent réussit
    And,
    /// || — exécuter le suivant seulement si le précédent échoue
    Or,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ParseError {
    UnexpectedToken(String),
    UnexpectedEof,
    EmptyCommand,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ParseError::UnexpectedToken(t) => write!(f, "rustshell: token inattendu: {}", t),
            ParseError::UnexpectedEof => write!(f, "rustshell: fin de ligne inattendue"),
            ParseError::EmptyCommand => write!(f, "rustshell: commande vide"),
        }
    }
}

/// Parse une séquence de tokens en CommandList.
pub fn parse(tokens: &[Token]) -> Result<CommandList, ParseError> {
    let mut pos = 0;
    let mut items = Vec::new();

    // Ignorer les tokens Eof en début
    skip_eof(tokens, &mut pos);

    if pos >= tokens.len() || tokens[pos] == Token::Eof {
        return Ok(CommandList { items });
    }

    loop {
        let pipeline = parse_pipeline(tokens, &mut pos)?;
        let connector = match tokens.get(pos) {
            Some(Token::Semicolon) => { pos += 1; Connector::Semicolon }
            Some(Token::And) => { pos += 1; Connector::And }
            Some(Token::Or) => { pos += 1; Connector::Or }
            Some(Token::Background) => {
                // & après un pipeline = background sur le pipeline entier
                pos += 1;
                // Marquer background sur la dernière commande
                let mut p = pipeline.clone();
                p.background = true;
                if let Some(last) = p.commands.last_mut() {
                    last.background = true;
                }
                items.push(PipelineItem { pipeline: p, connector: Connector::None });
                if tokens.get(pos) == Some(&Token::Eof) || pos >= tokens.len() {
                    break;
                }
                continue;
            }
            Some(Token::Eof) | None => Connector::None,
            Some(t) => return Err(ParseError::UnexpectedToken(format!("{:?}", t))),
        };

        let is_last = connector == Connector::None;
        items.push(PipelineItem { pipeline, connector });

        if is_last || pos >= tokens.len() || tokens[pos] == Token::Eof {
            break;
        }
    }

    Ok(CommandList { items })
}