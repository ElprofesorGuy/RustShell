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


/// Parse un pipeline (commandes reliées par |).
fn parse_pipeline(tokens: &[Token], pos: &mut usize) -> Result<Pipeline, ParseError> {
    let mut commands = Vec::new();

    let cmd = parse_command(tokens, pos)?;
    commands.push(cmd);

    while tokens.get(*pos) == Some(&Token::Pipe) {
        *pos += 1;
        let cmd = parse_command(tokens, pos)?;
        commands.push(cmd);
    }

    Ok(Pipeline { commands, background: false })
}

/// Parse une commande simple avec ses arguments et redirections.
fn parse_command(tokens: &[Token], pos: &mut usize) -> Result<Command, ParseError> {
    let mut argv = Vec::new();
    let mut redirects = Vec::new();

    loop {
        match tokens.get(*pos) {
            Some(Token::Word(w)) => {
                argv.push(w.clone());
                *pos += 1;
            }
            Some(Token::RedirectOut) => {
                *pos += 1;
                let file = expect_word(tokens, pos)?;
                redirects.push(Redirect {
                    kind: RedirectKind::Stdout,
                    target: RedirectTarget::File(file),
                });
            }
            Some(Token::RedirectAppend) => {
                *pos += 1;
                let file = expect_word(tokens, pos)?;
                redirects.push(Redirect {
                    kind: RedirectKind::Stdout,
                    target: RedirectTarget::FileAppend(file),
                });
            }
            Some(Token::RedirectIn) => {
                *pos += 1;
                let file = expect_word(tokens, pos)?;
                redirects.push(Redirect {
                    kind: RedirectKind::Stdin,
                    target: RedirectTarget::File(file),
                });
            }
            Some(Token::RedirectErr) => {
                *pos += 1;
                let file = expect_word(tokens, pos)?;
                redirects.push(Redirect {
                    kind: RedirectKind::Stderr,
                    target: RedirectTarget::File(file),
                });
            }
            Some(Token::RedirectErrOut) => {
                *pos += 1;
                let file = expect_word(tokens, pos)?;
                redirects.push(Redirect {
                    kind: RedirectKind::StdoutStderr,
                    target: RedirectTarget::File(file),
                });
            }
            // Fin de la commande courante
            _ => break,
        }
    }

    if argv.is_empty() {
        // Redirections seules sans commande = erreur
        if !redirects.is_empty() {
            return Err(ParseError::EmptyCommand);
        }
        return Err(ParseError::EmptyCommand);
    }

    Ok(Command { argv, redirects, background: false })
}

fn expect_word(tokens: &[Token], pos: &mut usize) -> Result<String, ParseError> {
    match tokens.get(*pos) {
        Some(Token::Word(w)) => {
            let w = w.clone();
            *pos += 1;
            Ok(w)
        }
        Some(_) => Err(ParseError::UnexpectedToken("attendu un nom de fichier".into())),
        None => Err(ParseError::UnexpectedEof),
    }
}

fn skip_eof(tokens: &[Token], pos: &mut usize) {
    while *pos < tokens.len() && tokens[*pos] == Token::Eof {
        *pos += 1;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::tokenize;

    fn parse_str(s: &str) -> Result<CommandList, ParseError> {
        let tokens = tokenize(s).expect("lex error");
        parse(&tokens)
    }

    #[test]
    fn test_simple_command() {
        let list = parse_str("ls -la").unwrap();
        assert_eq!(list.items.len(), 1);
        let cmd = &list.items[0].pipeline.commands[0];
        assert_eq!(cmd.argv, vec!["ls", "-la"]);
    }

    #[test]
    fn test_pipeline() {
        let list = parse_str("ls | grep foo").unwrap();
        let pipeline = &list.items[0].pipeline;
        assert_eq!(pipeline.commands.len(), 2);
        assert_eq!(pipeline.commands[0].argv[0], "ls");
        assert_eq!(pipeline.commands[1].argv[0], "grep");
    }

    #[test]
    fn test_redirect_out() {
        let list = parse_str("echo hi > out.txt").unwrap();
        let cmd = &list.items[0].pipeline.commands[0];
        assert_eq!(cmd.redirects.len(), 1);
        assert_eq!(cmd.redirects[0].kind, RedirectKind::Stdout);
    }

    #[test]
    fn test_semicolon_sequence() {
        let list = parse_str("echo a ; echo b").unwrap();
        assert_eq!(list.items.len(), 2);
        assert_eq!(list.items[0].connector, Connector::Semicolon);
    }

    #[test]
    fn test_and_connector() {
        let list = parse_str("mkdir dir && cd dir").unwrap();
        assert_eq!(list.items[0].connector, Connector::And);
    }

    #[test]
    fn test_empty_input() {
        let list = parse_str("").unwrap();
        assert_eq!(list.items.len(), 0);
    }
}
