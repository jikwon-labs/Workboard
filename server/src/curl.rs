// Running the system's curl (or what stands in for it): outgoing webhooks
// and slash commands (hooks.rs), iPhone pushes (push.rs) and the AI
// (ai.rs). Each caller picks its own flags; what goes on curl's stdin (a
// body, or a curl config carrying keys) never shows in its arguments.

use std::io::Write;
use std::process::{Child, Command, Output, Stdio};

/// Starts `program` with `args`, all three streams piped, and writes
/// `stdin` to it (then closes it). The caller reads stdout and waits.
pub fn spawn(program: &str, args: &[&str], stdin: &[u8]) -> std::io::Result<Child> {
    let mut child = Command::new(program)
        .args(args)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()?;
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(stdin);
    }
    Ok(child)
}

/// Runs `program` to the end with `stdin` and gives back what it wrote and
/// how it ended. Err only when it couldn't be started or waited for.
pub fn run(program: &str, args: &[&str], stdin: &[u8]) -> Result<Output, String> {
    let child =
        spawn(program, args, stdin).map_err(|e| format!("couldn't run {}: {}", program, e))?;
    child.wait_with_output().map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn runs_with_stdin_and_keeps_both_outputs() {
        let out = run("sh", &["-c", "cat; echo oops >&2; exit 3"], b"hello").unwrap();
        assert_eq!(out.stdout, b"hello");
        assert_eq!(String::from_utf8_lossy(&out.stderr).trim(), "oops");
        assert_eq!(out.status.code(), Some(3));
    }

    #[test]
    fn says_what_couldnt_be_started() {
        let e = run("wb-no-such-program", &[], b"").unwrap_err();
        assert!(e.starts_with("couldn't run wb-no-such-program: "), "{}", e);
    }
}
