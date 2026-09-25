//! UCI protocol adapter. It contains no chess logic: it drives the shared
//! `Engine` and `Position` APIs used by the command-line analyzer.

use std::io::{self, BufRead, Write};
use std::sync::{mpsc, Arc, atomic::AtomicBool};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use crate::moves::parse_uci_move;
use crate::search::{format_score, Engine, SearchLimits, SearchResult};
use crate::{Position, STARTPOS_FEN};

struct Active { stop: Arc<AtomicBool>, join: JoinHandle<()> }

pub fn run() -> io::Result<()> {
    let (input_tx, input_rx) = mpsc::channel::<String>();
    thread::spawn(move || { let stdin = io::stdin(); for line in stdin.lock().lines() { match line { Ok(s) => { if input_tx.send(s).is_err() { break; } }, Err(_) => break } } });
    let (result_tx, result_rx) = mpsc::channel::<(Engine, SearchResult)>();
    let mut stdout = io::BufWriter::new(io::stdout()); let mut position = Position::from_fen(STARTPOS_FEN).map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
    let mut engine = Some(Engine::default()); let mut multipv = 1_usize; let mut active: Option<Active> = None;
    let mut quit = false;
    while !quit {
        if let Ok((returned_engine, result)) = result_rx.try_recv() {
            engine = Some(returned_engine); if let Some(a) = active.take() { let _ = a.join.join(); }
            emit_result(&mut stdout, &result)?;
        }
        match input_rx.recv_timeout(Duration::from_millis(10)) {
            Ok(line) => {
                let words = line.split_ascii_whitespace().collect::<Vec<_>>(); if words.is_empty() { continue; }
                match words[0] {
                    "uci" => { write_line(&mut stdout, "id name Correctness First Rust Chess"); write_line(&mut stdout, "id author TheronRadley contributors"); write_line(&mut stdout, "option name Hash type spin default 16 min 1 max 4096"); write_line(&mut stdout, "option name MultiPV type spin default 1 min 1 max 5"); write_line(&mut stdout, "option name Threads type spin default 1 min 1 max 1"); write_line(&mut stdout, "uciok"); stdout.flush()?; }
                    "isready" => { write_line(&mut stdout, "readyok"); stdout.flush()?; }
                    "ucinewgame" => { if let Some(e) = engine.as_mut() { e.clear_hash(); } }
                    "setoption" => { if let Err(e) = set_option(&words, engine.as_mut(), &mut multipv) { write_line(&mut stdout, &format!("info string error: {e}")); stdout.flush()?; } }
                    "position" => match parse_position(&words[1..]) { Ok(p) => position = p, Err(e) => { write_line(&mut stdout, &format!("info string error: {e}")); stdout.flush()?; } },
                    "go" => {
                        if active.is_some() { write_line(&mut stdout, "info string error: search already running; send stop first"); stdout.flush()?; continue; }
                        let limits = match parse_go(&words[1..], position.side_to_move(), multipv) { Ok(v) => v, Err(e) => { write_line(&mut stdout, &format!("info string error: {e}")); stdout.flush()?; continue; } };
                        let worker_engine = match engine.take() { Some(e) => e, None => { write_line(&mut stdout, "info string error: engine unavailable"); stdout.flush()?; continue; } };
                        let mut worker_position = position.clone(); let stop = Arc::new(AtomicBool::new(false)); let stop_for_thread = Arc::clone(&stop); let tx = result_tx.clone();
                        write_line(&mut stdout, "info string searching"); stdout.flush()?;
                        let join = thread::spawn(move || { let mut e = worker_engine; let result = e.search(&mut worker_position, limits, Some(stop_for_thread)); let _ = tx.send((e, result)); });
                        active = Some(Active { stop, join });
                    }
                    "stop" => { if let Some(a) = active.as_ref() { a.stop.store(true, std::sync::atomic::Ordering::Relaxed); } }
                    "quit" => { if let Some(a) = active.as_ref() { a.stop.store(true, std::sync::atomic::Ordering::Relaxed); } if let Some(a) = active.take() { let _ = a.join.join(); } quit = true; }
                    "debug" => { write_line(&mut stdout, "info string debug diagnostics are reported in final search info"); stdout.flush()?; }
                    _ => { write_line(&mut stdout, &format!("info string error: unsupported command `{}`", words[0])); stdout.flush()?; }
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => { if let Some(a) = active.as_ref() { a.stop.store(true, std::sync::atomic::Ordering::Relaxed); } if let Some(a) = active.take() { let _ = a.join.join(); } break; }
        }
    }
    Ok(())
}

fn write_line(out: &mut impl Write, text: &str) { let _ = writeln!(out, "{text}"); }
fn emit_result(out: &mut impl Write, result: &SearchResult) -> io::Result<()> {
    for line in &result.lines {
        let score = format_score(line.score); let (kind, value) = score.split_once(' ').unwrap_or(("cp", "0"));
        write_line(out, &format!("info depth {} seldepth {} multipv {} score {} {} nodes {} nps {} hashfull {} time {} pv {}", line.depth, result.seldepth, line.rank, kind, value, result.nodes, result.nps, result.hashfull, result.elapsed.as_millis(), line.pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>().join(" ")));
    }
    let best = result.best_move().map(|m| m.to_uci()).unwrap_or_else(|| "0000".into()); write_line(out, &format!("bestmove {best}")); out.flush()
}

fn set_option(words: &[&str], engine: Option<&mut Engine>, multipv: &mut usize) -> Result<(), String> {
    if words.len() < 4 || !words[0].eq_ignore_ascii_case("name") { return Err("setoption syntax is `setoption name <Name> value <Value>`".into()); }
    let value_at = words.iter().position(|s| s.eq_ignore_ascii_case("value")).ok_or_else(|| "setoption requires `value`".to_owned())?;
    let name = words[1..value_at].join(" "); let value = *words.get(value_at + 1).ok_or_else(|| "setoption value is missing".to_owned())?;
    match name.to_ascii_lowercase().as_str() {
        "hash" => { let mb = value.parse::<usize>().map_err(|_| "Hash must be an integer".to_owned())?.clamp(1, 4096); engine.ok_or_else(|| "cannot change Hash during search".to_owned())?.set_hash_mb(mb); }
        "multipv" => { *multipv = value.parse::<usize>().map_err(|_| "MultiPV must be an integer".to_owned())?.clamp(1, 5); if let Some(e) = engine { e.set_default_multipv(*multipv); } }
        "threads" => { if value != "1" { return Err("this correctness-first build is single-threaded; Threads is fixed at 1".into()); } }
        _ => return Err(format!("unknown option `{name}`")),
    }
    Ok(())
}

fn parse_position(words: &[&str]) -> Result<Position, String> {
    if words.is_empty() { return Err("position needs `startpos` or `fen`".into()); }
    let (mut position, move_start) = match words[0] {
        "startpos" => (Position::from_fen(STARTPOS_FEN).map_err(|e| e.to_string())?, 1),
        "fen" => {
            if words.len() < 7 { return Err("position fen needs exactly six FEN fields".into()); }
            (Position::from_fen(&words[1..7].join(" ")).map_err(|e| e.to_string())?, 7)
        }
        _ => return Err("position needs `startpos` or `fen`".into()),
    };
    if move_start < words.len() && words[move_start] != "moves" { return Err("expected `moves` after position".into()); }
    if move_start < words.len() { for text in &words[move_start + 1..] { let mv = parse_uci_move(&position, text).map_err(|e| e.to_string())?; position.try_make_move(mv).map_err(|e| e.to_string())?; } }
    Ok(position)
}

fn parse_go(words: &[&str], side: crate::board::Color, multipv: usize) -> Result<SearchLimits, String> {
    let mut limits = SearchLimits { depth: None, movetime: None, nodes: None, multipv }; let mut wtime = None; let mut btime = None; let mut winc = 0_u64; let mut binc = 0_u64; let mut i = 0;
    while i < words.len() {
        let need = |i: usize| words.get(i + 1).ok_or_else(|| format!("go {} needs a value", words[i]));
        match words[i] {
            "depth" => { limits.depth = Some(need(i)?.parse().map_err(|_| "go depth must be an integer".to_owned())?); i += 2; }
            "movetime" => { limits.movetime = Some(Duration::from_millis(need(i)?.parse().map_err(|_| "go movetime must be milliseconds".to_owned())?)); i += 2; }
            "nodes" => { limits.nodes = Some(need(i)?.parse().map_err(|_| "go nodes must be an integer".to_owned())?); i += 2; }
            "wtime" => { wtime = Some(need(i)?.parse::<u64>().map_err(|_| "go wtime must be milliseconds".to_owned())?); i += 2; }
            "btime" => { btime = Some(need(i)?.parse::<u64>().map_err(|_| "go btime must be milliseconds".to_owned())?); i += 2; }
            "winc" => { winc = need(i)?.parse().map_err(|_| "go winc must be milliseconds".to_owned())?; i += 2; }
            "binc" => { binc = need(i)?.parse().map_err(|_| "go binc must be milliseconds".to_owned())?; i += 2; }
            "infinite" => { i += 1; }
            "ponder" => { i += 1; }
            unknown => return Err(format!("unsupported go token `{unknown}`")),
        }
    }
    if limits.movetime.is_none() {
        let (remain, inc) = if side == crate::board::Color::White { (wtime, winc) } else { (btime, binc) };
        if let Some(remain) = remain { // Conservative hard allocation: 1/30 plus most increment, capped below remaining clock.
            let soft = remain / 30 + inc.saturating_mul(3) / 4; let hard = soft.saturating_mul(3).min(remain.saturating_sub(5)).max(1); limits.movetime = Some(Duration::from_millis(hard));
        }
    }
    if limits.depth.is_none() && limits.movetime.is_none() && limits.nodes.is_none() { limits.depth = Some(64); }
    Ok(limits)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_position_and_rejects_bad_move() {
        let p = parse_position(&["startpos", "moves", "e2e4", "e7e5"]).expect("legal UCI game");
        assert_eq!(p.to_fen(), "rnbqkbnr/pppp1ppp/8/4p3/4P3/8/PPPP1PPP/RNBQKBNR w KQkq e6 0 2");
        assert!(parse_position(&["startpos", "moves", "e2e5"]).is_err());
    }
    #[test]
    fn parses_go_clock_and_limits() {
        let go = parse_go(&["wtime", "30000", "btime", "25000", "winc", "500", "depth", "12"], crate::board::Color::White, 2).expect("go");
        assert_eq!(go.depth, Some(12)); assert_eq!(go.multipv, 2); assert!(go.movetime.is_some());
    }
}
