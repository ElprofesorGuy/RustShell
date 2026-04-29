// Module analyseur lexical (lexer) pour le shell RustShell
//
// Tâches à implémenter :
// - Définir une énumération pour les types de tokens (par exemple : Word, Pipe, Ampersand, Semicolon, etc.)

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    /// Mot ordinaire : commande, argument, chemin
    Word(String),
    /// Opérateur pipe |
    Pipe,
    /// Redirection stdout >
    RedirectOut,
    /// Redirection stdout en mode append >>
    RedirectAppend,
    /// Redirection stdin <
    RedirectIn,
    /// Redirection stderr 2>
    RedirectErr,
    /// Redirection stderr + stdout 2>&1 ou &>
    RedirectErrOut,
    /// Arrière-plan &
    Background,
    /// Séparateur de commandes ;
    Semicolon,
    /// Opérateur logique &&
    And,
    /// Opérateur logique ||
    Or,
    /// Fin de ligne / input
    Eof,
}

#[derive(Debug, Clone, PartialEq)]
pub enum LexError {
    UnterminatedString,
    UnexpectedChar(char),
}

impl std::fmt::Display for LexError{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LexError::UnterminatedString => write!(f, "rustshell: chaîne non terminée"),
            LexError::UnexpectedChar(c) => write!(f, "rustshell: caractère inattendu: '{}'", c),
        }
    }
}
// - Implémenter une fonction de tokenisation qui prend une chaîne de caractères (ligne de commande)
//   et retourne un vecteur de tokens
pub fn tokenize(input: &str) -> Result<Vec<Token>, LexError> {
    let mut tokens = Vec::new();
    let mut chars = input.chars().peekable();

    while let Some(&c) = chars.peek() {
        match c {
            // Espaces ignorés entre tokens
            ' ' | '\t' | '\r' => {
                chars.next();
            }

            // Commentaire : ignore le reste de la ligne
            '#' => break,

            // Pipe ou arrière-plan
            '|' => {
                chars.next();
                if chars.peek() == Some(&'|') {
                    chars.next();
                    tokens.push(Token::Or);
                } else {
                    tokens.push(Token::Pipe);
                }
            }

            '&' => {
                chars.next();
                if chars.peek() == Some(&'&') {
                    chars.next();
                    tokens.push(Token::And);
                } else if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirectErrOut);
                } else {
                    tokens.push(Token::Background);
                }
            }

            ';' => {
                chars.next();
                tokens.push(Token::Semicolon);
            }

            // Redirections
            '>' => {
                chars.next();
                if chars.peek() == Some(&'>') {
                    chars.next();
                    tokens.push(Token::RedirectAppend);
                } else {
                    tokens.push(Token::RedirectOut);
                }
            }

            '<' => {
                chars.next();
                tokens.push(Token::RedirectIn);
            }

            '2' => {
                // Lookahead pour 2> et 2>>
                let mut lookahead = input[input.len() - chars.clone().collect::<String>().len()..].chars();
                lookahead.next(); // consume '2'
                if lookahead.next() == Some('>') {
                    // C'est une redirection stderr
                    chars.next(); // '2'
                    chars.next(); // '>'
                    if chars.peek() == Some(&'>') {
                        chars.next();
                        // 2>> : on traite comme RedirectErr + Append (simplifié -> RedirectErr)
                        tokens.push(Token::RedirectErr);
                    } else if chars.peek() == Some(&'&') {
                        chars.next();
                        if chars.peek() == Some(&'1') {
                            chars.next();
                        }
                        tokens.push(Token::RedirectErrOut);
                    } else {
                        tokens.push(Token::RedirectErr);
                    }
                } else {
                    // C'est juste le chiffre '2' dans un mot
                    let word = read_word(&mut chars)?;
                    tokens.push(Token::Word(word));
                }
            }

            // Guillemets doubles : interpolation des variables (simplifiée)
            '"' => {
                chars.next();
                let word = read_double_quoted(&mut chars)?;
                tokens.push(Token::Word(word));
            }

            // Guillemets simples : littéral strict
            '\'' => {
                chars.next();
                let word = read_single_quoted(&mut chars)?;
                tokens.push(Token::Word(word));
            }

            // Mot ordinaire
            _ => {
                let word = read_word(&mut chars)?;
                tokens.push(Token::Word(word));
            }
        }
    }

    tokens.push(Token::Eof);
    Ok(tokens)
}

/// Lit un mot jusqu'au prochain séparateur ou opérateur.
fn read_word(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, LexError> {
    let mut word = String::new();

    while let Some(&c) = chars.peek() {
        match c {
            ' ' | '\t' | '\r' | '|' | '&' | ';' | '<' | '>' | '#' => break,
            '\\' => {
                chars.next();
                if let Some(escaped) = chars.next() {
                    word.push(escaped);
                }
            }
            '"' => {
                chars.next();
                let inner = read_double_quoted(chars)?;
                word.push_str(&inner);
            }
            '\'' => {
                chars.next();
                let inner = read_single_quoted(chars)?;
                word.push_str(&inner);
            }
            _ => {
                word.push(c);
                chars.next();
            }
        }
    }

    Ok(word)
}
// - Gérer les espaces, les guillemets pour les arguments avec espaces, et les opérateurs spéciaux
// - Traiter les erreurs de syntaxe de base (tokens invalides)

/// Lit le contenu d'une chaîne à guillemets doubles jusqu'au prochain `"`.
fn read_double_quoted(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, LexError> {
    let mut s = String::new();
    loop {
        match chars.next() {
            None => return Err(LexError::UnterminatedString),
            Some('"') => break,
            Some('\\') => {
                match chars.next() {
                    None => return Err(LexError::UnterminatedString),
                    Some(c) => s.push(c),
                }
            }
            Some(c) => s.push(c),
        }
    }
    Ok(s)
}

/// Lit le contenu d'une chaîne à guillemets simples jusqu'au prochain `'`.
fn read_single_quoted(chars: &mut std::iter::Peekable<std::str::Chars>) -> Result<String, LexError> {
    let mut s = String::new();
    loop {
        match chars.next() {
            None => return Err(LexError::UnterminatedString),
            Some('\'') => break,
            Some(c) => s.push(c),
        }
    }
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_command() {
        let tokens = tokenize("ls -la").unwrap();
        assert_eq!(tokens[0], Token::Word("ls".into()));
        assert_eq!(tokens[1], Token::Word("-la".into()));
        assert_eq!(tokens[2], Token::Eof);
    }

    #[test]
    fn test_pipe() {
        let tokens = tokenize("ls | grep foo").unwrap();
        assert_eq!(tokens[1], Token::Pipe);
    }

    #[test]
    fn test_redirect_out() {
        let tokens = tokenize("echo hi > file.txt").unwrap();
        assert_eq!(tokens[2], Token::RedirectOut);
        assert_eq!(tokens[3], Token::Word("file.txt".into()));
    }

    #[test]
    fn test_redirect_append() {
        let tokens = tokenize("echo hi >> file.txt").unwrap();
        assert_eq!(tokens[2], Token::RedirectAppend);
    }

    #[test]
    fn test_background() {
        let tokens = tokenize("sleep 10 &").unwrap();
        assert_eq!(tokens[2], Token::Background);
    }

    #[test]
    fn test_double_quoted_string() {
        let tokens = tokenize("echo \"hello world\"").unwrap();
        assert_eq!(tokens[1], Token::Word("hello world".into()));
    }

    #[test]
    fn test_single_quoted_string() {
        let tokens = tokenize("echo 'hello world'").unwrap();
        assert_eq!(tokens[1], Token::Word("hello world".into()));
    }

    #[test]
    fn test_unterminated_string() {
        assert!(tokenize("echo \"unterminated").is_err());
    }

    #[test]
    fn test_comment() {
        let tokens = tokenize("ls # this is a comment").unwrap();
        assert_eq!(tokens[0], Token::Word("ls".into()));
        assert_eq!(tokens[1], Token::Eof);
    }

    #[test]
    fn test_and_or() {
        let tokens = tokenize("cmd1 && cmd2 || cmd3").unwrap();
        assert_eq!(tokens[1], Token::And);
        assert_eq!(tokens[3], Token::Or);
    }
}
