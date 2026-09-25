use std::io::Write;
use std::process::{Command, Stdio};
use std::thread;
use std::time::Duration;

#[test]
fn uci_handshake_position_go_and_bestmove() {
    let exe = env!("CARGO_BIN_EXE_chess-uci");
    let mut child = Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("spawn UCI binary");
    let mut input = child.stdin.take().expect("stdin");
    writeln!(input, "uci").unwrap(); writeln!(input, "isready").unwrap(); writeln!(input, "position startpos moves e2e4 e7e5").unwrap(); writeln!(input, "go depth 1").unwrap(); input.flush().unwrap();
    thread::sleep(Duration::from_millis(250)); writeln!(input, "quit").unwrap(); drop(input);
    let output = child.wait_with_output().expect("UCI exits"); let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("uciok"), "{text}"); assert!(text.contains("readyok"), "{text}"); assert!(text.contains("bestmove "), "{text}");
}

#[test]
fn malformed_uci_position_is_reported_without_crash() {
    let exe = env!("CARGO_BIN_EXE_chess-uci");
    let mut child = Command::new(exe).stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().expect("spawn UCI binary"); let mut input = child.stdin.take().expect("stdin");
    writeln!(input, "position fen nonsense").unwrap(); writeln!(input, "quit").unwrap(); drop(input);
    let output = child.wait_with_output().expect("UCI exits"); assert!(String::from_utf8_lossy(&output.stdout).contains("info string error"));
}
