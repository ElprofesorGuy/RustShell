/// Module terminal : lecture en mode raw avec support des flèches, Tab, Ctrl+C, Ctrl+R.
///
/// Nouveautés v2 :
///   - Ctrl+R : recherche incrémentale dans l'historique (reverse-search)
///   - prompt_visible_len() : calcule la longueur visible en ignorant les codes ANSI
///   - redraw_line mis à jour pour les prompts colorés multi-octets


use std::io::{self, Read, Write};
use libc::{tcgetattr, tcsetattr, termios, TCSANOW, ECHO, ICANON, VMIN, VTIME};

use crate::history::{History, complete, common_prefix};

/// Résultat de la lecture d'une ligne.
pub enum ReadLine {
    Line(String),
    Eof,
    Interrupted,
}

/// Calcule la longueur *visible* d'un prompt en ignorant les séquences ANSI \x1b[...m
pub fn prompt_visible_len(prompt: &str) -> usize {
    let mut len = 0;
    let mut in_escape = false;
    for c in prompt.chars() {
        if c == '\x1b' {
            in_escape = true;
        } else if in_escape {
            if c == 'm' || c == 'K' || c == 'J' || c == 'H' || c == 'A'
                || c == 'B' || c == 'C' || c == 'D' {
                in_escape = false;
            }
            // reste en escape
        } else {
            // Compte les caractères multi-octets comme 1 colonne (approximation)
            if c as u32 <= 0x7F || c as u32 >= 0x80 {
                len += 1;
            }
        }
    }
    len
}

/// Lit une ligne de l'utilisateur en mode raw avec support complet du terminal.
pub fn readline(prompt: &str, history: &mut History) -> ReadLine {
    let stdout = io::stdout();
    let mut out = stdout.lock();

    let _ = write!(out, "{}", prompt);
    let _ = out.flush();

    let old_termios = match enter_raw_mode() {
        Ok(t) => t,
        Err(_) => return readline_simple(prompt),
    };

    let visible_len = prompt_visible_len(prompt);
    let mut line   = String::new();
    let mut cursor = 0usize;

    let result = loop {
        let byte = match read_byte() {
            Ok(b) => b,
            Err(_) => break ReadLine::Eof,
        };

        match byte {
            // Ctrl+D : EOF si ligne vide, sinon supprimer caractère
            4 => {
                if line.is_empty() {
                    let _ = writeln!(out);
                    break ReadLine::Eof;
                }
                if cursor < line.len() {
                    line.remove(cursor);
                    redraw_line(&mut out, &line, cursor, visible_len);
                }
            }

            // Ctrl+C : annuler
            3 => {
                let _ = writeln!(out, "^C");
                let _ = out.flush();
                line.clear();
                cursor = 0;
                break ReadLine::Interrupted;
            }

            // Ctrl+R : recherche inverse dans l'historique
            18 => {
                restore_terminal(&old_termios);
                let found = reverse_search(&mut out, history);
                let _ = enter_raw_mode(); // re-enter raw après la recherche
                if let Some(entry) = found {
                    line = entry;
                    cursor = line.len();
                    // Réafficher le prompt original + la ligne trouvée
                    let _ = write!(out, "\r\x1b[2K{}{}", prompt, line);
                    let _ = out.flush();
                } else {
                    let _ = write!(out, "\r\x1b[2K{}{}", prompt, line);
                    let _ = out.flush();
                }
                // Re-enter raw mode
                let _ = enter_raw_mode();
            }

            // Ctrl+A : début de ligne
            1 => {
                cursor = 0;
                redraw_line(&mut out, &line, cursor, visible_len);
            }

            // Ctrl+E : fin de ligne
            5 => {
                cursor = line.len();
                redraw_line(&mut out, &line, cursor, visible_len);
            }

            // Ctrl+K : supprimer jusqu'à la fin de ligne
            11 => {
                line.truncate(cursor);
                redraw_line(&mut out, &line, cursor, visible_len);
            }

            // Ctrl+U : supprimer jusqu'au début de ligne
            21 => {
                line = line[cursor..].to_string();
                cursor = 0;
                redraw_line(&mut out, &line, cursor, visible_len);
            }

            // Ctrl+W : supprimer le mot précédent
            23 => {
                if cursor > 0 {
                    let word_start = find_word_start(&line, cursor);
                    line.replace_range(word_start..cursor, "");
                    cursor = word_start;
                    redraw_line(&mut out, &line, cursor, visible_len);
                }
            }

            // Entrée
            b'\n' | b'\r' => {
                let _ = writeln!(out);
                let _ = out.flush();
                break ReadLine::Line(line);
            }

            // Backspace
            127 | 8 => {
                if cursor > 0 {
                    cursor -= 1;
                    line.remove(cursor);
                    redraw_line(&mut out, &line, cursor, visible_len);
                }
            }

            // Tab : autocomplétion
            b'\t' => {
                let word_start = find_word_start(&line, cursor);
                let prefix = line[word_start..cursor].to_string();
                let is_first_word = line[..word_start].trim().is_empty();
                let completions = complete(&prefix, is_first_word);

                if completions.len() == 1 {
                    let suffix = completions[0][prefix.len()..].to_string();
                    for ch in suffix.chars() {
                        line.insert(cursor, ch);
                        cursor += 1;
                    }
                    redraw_line(&mut out, &line, cursor, visible_len);
                } else if completions.len() > 1 {
                    let common = common_prefix(&completions);
                    if common.len() > prefix.len() {
                        let suffix = common[prefix.len()..].to_string();
                        for ch in suffix.chars() {
                            line.insert(cursor, ch);
                            cursor += 1;
                        }
                        redraw_line(&mut out, &line, cursor, visible_len);
                    } else {
                        let _ = writeln!(out);
                        // Afficher en colonnes
                        print_columns(&mut out, &completions);
                        let _ = write!(out, "{}{}", prompt, line);
                        let offset = line.len() - cursor;
                        if offset > 0 { let _ = write!(out, "\x1b[{}D", offset); }
                        let _ = out.flush();
                    }
                }
            }

            // Séquences ESC
            27 => {
                if read_byte().ok() == Some(b'[') {
                    match read_byte().ok() {
                        Some(b'A') => {
                            if let Some(entry) = history.prev(&line) {
                                line = entry.to_string();
                                cursor = line.len();
                                redraw_line(&mut out, &line, cursor, visible_len);
                            }
                        }
                        Some(b'B') => {
                            if let Some(entry) = history.next() {
                                line = entry.to_string();
                                cursor = line.len();
                                redraw_line(&mut out, &line, cursor, visible_len);
                            }
                        }
                        Some(b'C') => {
                            if cursor < line.len() {
                                cursor += 1;
                                let _ = write!(out, "\x1b[C");
                                let _ = out.flush();
                            }
                        }
                        Some(b'D') => {
                            if cursor > 0 {
                                cursor -= 1;
                                let _ = write!(out, "\x1b[D");
                                let _ = out.flush();
                            }
                        }
                        Some(b'1') => {
                            read_byte().ok();
                            cursor = 0;
                            redraw_line(&mut out, &line, cursor, visible_len);
                        }
                        Some(b'4') | Some(b'F') => {
                            if let Some(b'4') = Some(b'4') { read_byte().ok(); }
                            cursor = line.len();
                            redraw_line(&mut out, &line, cursor, visible_len);
                        }
                        Some(b'3') => {
                            read_byte().ok();
                            if cursor < line.len() {
                                line.remove(cursor);
                                redraw_line(&mut out, &line, cursor, visible_len);
                            }
                        }
                        // Ctrl+flèche droite : mot suivant
                        Some(b'1') => {
                            read_byte().ok(); // ';'
                            read_byte().ok(); // '5'
                            if let Some(b'C') = read_byte().ok() {
                                while cursor < line.len() && line.as_bytes()[cursor] == b' ' { cursor += 1; }
                                while cursor < line.len() && line.as_bytes()[cursor] != b' ' { cursor += 1; }
                                redraw_line(&mut out, &line, cursor, visible_len);
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Caractère imprimable
            b if b >= 32 && b < 127 => {
                let ch = b as char;
                line.insert(cursor, ch);
                cursor += 1;
                history.reset_nav();
                redraw_line(&mut out, &line, cursor, visible_len);
            }

            _ => {}
        }
    };

    restore_terminal(&old_termios);
    result
}

/// Recherche incrémentale inverse dans l'historique (Ctrl+R).
/// Retourne la ligne sélectionnée ou None si annulée.
fn reverse_search(out: &mut impl Write, history: &mut History) -> Option<String> {
    let mut query = String::new();

    loop {
        // Afficher le prompt de recherche
        let current = history.search(&query);
        let display = current.unwrap_or("");
        let _ = write!(out, "\r\x1b[2K(reverse-search)`{}': {}", query, display);
        let _ = out.flush();

        // Lire en mode raw (déjà restauré à ce point, relire via stdin)
        let old = match enter_raw_mode() {
            Ok(t) => t,
            Err(_) => return None,
        };
        let b = read_byte().ok();
        restore_terminal(&old);

        match b {
            Some(b'\n') | Some(b'\r') => {
                // Valider la sélection courante
                let _ = writeln!(out);
                return current.map(|s| s.to_string());
            }
            Some(3) | Some(7) => {
                // Ctrl+C ou Ctrl+G : annuler
                let _ = writeln!(out);
                return None;
            }
            Some(127) | Some(8) => {
                // Backspace : effacer dernier char de la query
                if !query.is_empty() {
                    query.pop();
                }
            }
            Some(b) if b >= 32 && b < 127 => {
                query.push(b as char);
            }
            _ => {
                let _ = writeln!(out);
                return current.map(|s| s.to_string());
            }
        }
    }
}

/// Affiche les complétions en colonnes bien alignées.
fn print_columns(out: &mut impl Write, items: &[String]) {
    if items.is_empty() { return; }
    let max_len = items.iter().map(|s| s.len()).max().unwrap_or(0) + 2;
    let cols = (80 / max_len).max(1);
    for (i, item) in items.iter().enumerate() {
        let _ = write!(out, "{:<width$}", item, width = max_len);
        if (i + 1) % cols == 0 { let _ = writeln!(out); }
    }
    if items.len() % cols != 0 { let _ = writeln!(out); }
}

/// Redessine la ligne courante (fonctionne avec prompts colorés ANSI).
fn redraw_line(out: &mut impl Write, line: &str, cursor: usize, visible_prompt_len: usize) {
    // \r : retour début de ligne
    // \x1b[{n}C : avancer de n colonnes (sauter le prompt)
    // \x1b[K : effacer jusqu'à la fin
    let _ = write!(out, "\r\x1b[{}C\x1b[K{}", visible_prompt_len, line);
    let offset = line.len() - cursor;
    if offset > 0 {
        let _ = write!(out, "\x1b[{}D", offset);
    }
    let _ = out.flush();
}

fn find_word_start(line: &str, cursor: usize) -> usize {
    let bytes = &line.as_bytes()[..cursor];
    bytes.iter().rposition(|&b| b == b' ' || b == b'\t')
        .map(|p| p + 1)
        .unwrap_or(0)
}

fn read_byte() -> io::Result<u8> {
    let mut buf = [0u8; 1];
    io::stdin().lock().read_exact(&mut buf)?;
    Ok(buf[0])
}

fn enter_raw_mode() -> io::Result<termios> {
    unsafe {
        let mut old = std::mem::zeroed::<termios>();
        if tcgetattr(libc::STDIN_FILENO, &mut old) != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut raw = old;
        raw.c_lflag &= !(ECHO | ICANON);
        raw.c_cc[VMIN] = 1;
        raw.c_cc[VTIME] = 0;
        if tcsetattr(libc::STDIN_FILENO, TCSANOW, &raw) != 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(old)
    }
}

fn restore_terminal(old: &termios) {
    unsafe { tcsetattr(libc::STDIN_FILENO, TCSANOW, old); }
}

fn readline_simple(prompt: &str) -> ReadLine {
    let _ = print!("{}", prompt);
    let _ = io::stdout().flush();
    let mut line = String::new();
    match io::stdin().read_line(&mut line) {
        Ok(0) => ReadLine::Eof,
        Ok(_) => ReadLine::Line(line.trim_end_matches('\n').trim_end_matches('\r').to_string()),
        Err(_) => ReadLine::Eof,
    }
}