//! Terminal der CLI: Eingaben (Passwörter/TANs verdeckt), Hinweise auf stderr, Ergebnisse auf stdout.

#[cfg(test)]
use std::collections::VecDeque;
use std::io::{BufRead, IsTerminal, Write};
#[cfg(test)]
use std::sync::Mutex;

use zeroize::Zeroizing;

/// Ein- und Ausgabe der Kommandozeile.
pub trait Terminal: Send + Sync {
    /// Rückfragen möglich (stdin und stderr sind ein Terminal)?
    fn interactive(&self) -> bool;
    /// Zeile lesen (`None` bei EOF).
    fn line(&self, prompt: &str) -> Option<String>;
    /// Verdeckt lesen (Passwort, TAN).
    fn secret(&self, prompt: &str) -> Option<Zeroizing<String>>;
    /// Hinweis (stderr).
    fn info(&self, msg: &str);
    /// Ergebnis (stdout).
    fn out(&self, msg: &str);
}

/// Echtes Terminal.
#[derive(Debug, Default)]
pub struct Tty;

impl Terminal for Tty {
    fn interactive(&self) -> bool {
        std::io::stdin().is_terminal() && std::io::stderr().is_terminal()
    }

    fn line(&self, prompt: &str) -> Option<String> {
        eprint!("{prompt}");
        let _ = std::io::stderr().flush();
        let mut s = String::new();
        match std::io::stdin().lock().read_line(&mut s) {
            Ok(0) | Err(_) => None,
            Ok(_) => Some(s.trim_end_matches(['\r', '\n']).to_owned()),
        }
    }

    fn secret(&self, prompt: &str) -> Option<Zeroizing<String>> {
        if !std::io::stdin().is_terminal() {
            return self.line(prompt).map(Zeroizing::new);
        }
        rpassword::prompt_password(prompt).ok().map(Zeroizing::new)
    }

    fn info(&self, msg: &str) {
        eprintln!("{msg}");
    }

    fn out(&self, msg: &str) {
        // Schreibfehler (z. B. geschlossene Pipe bei `pida-cli licenses | head`) nicht als Panik: die Ausgabe ist nur
        // Bericht, der Ablauf (und sein Exit-Code) geht weiter
        let _ = writeln!(std::io::stdout().lock(), "{msg}");
    }
}

/// Skriptgesteuertes Terminal (Tests).
#[cfg(test)]
#[derive(Debug, Default)]
pub struct Scripted {
    /// Antworten in Reihenfolge
    pub answers: Mutex<VecDeque<String>>,
    /// Prompts
    pub prompts: Mutex<Vec<String>>,
    /// stderr
    pub err: Mutex<Vec<String>>,
    /// stdout
    pub out: Mutex<Vec<String>>,
    /// interaktiv
    pub interactive: bool,
}

#[cfg(test)]
impl Scripted {
    /// Neues Terminal mit Antworten.
    pub fn new(interactive: bool, answers: &[&str]) -> Scripted {
        Scripted {
            answers: Mutex::new(answers.iter().map(|s| (*s).to_owned()).collect()),
            interactive,
            ..Scripted::default()
        }
    }

    fn next(&self, prompt: &str) -> Option<String> {
        self.prompts.lock().unwrap().push(prompt.to_owned());
        self.answers.lock().unwrap().pop_front()
    }

    /// stdout gesamt.
    pub fn stdout(&self) -> String {
        self.out.lock().unwrap().join("\n")
    }

    /// stderr gesamt.
    pub fn stderr(&self) -> String {
        self.err.lock().unwrap().join("\n")
    }
}

#[cfg(test)]
impl Terminal for Scripted {
    fn interactive(&self) -> bool {
        self.interactive
    }
    fn line(&self, prompt: &str) -> Option<String> {
        self.next(prompt)
    }
    fn secret(&self, prompt: &str) -> Option<Zeroizing<String>> {
        self.next(prompt).map(Zeroizing::new)
    }
    fn info(&self, msg: &str) {
        self.err.lock().unwrap().push(msg.to_owned());
    }
    fn out(&self, msg: &str) {
        self.out.lock().unwrap().push(msg.to_owned());
    }
}
