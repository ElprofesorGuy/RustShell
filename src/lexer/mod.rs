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
