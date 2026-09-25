use std::env;
use std::process::ExitCode;
use std::time::Duration;

use chess_engine::{perft, moves, Engine, Position, SearchLimits, STARTPOS_FEN};

fn usage() {
    eprintln!("Usage:\n  chess-engine analyze --fen <FEN> [--depth N] [--movetime MS] [--nodes N] [--multipv K] [--hash MB] [--debug]\n  chess-engine perft --fen <FEN> --depth N [--divide]\n\nIf --fen is omitted, startpos is used.");
}
fn value<'a>(args: &'a [String], i: &mut usize, flag: &str) -> Result<&'a str, String> { *i += 1; args.get(*i).map(String::as_str).ok_or_else(|| format!("{flag} needs a value")) }
fn parse_args(args: &[String]) -> Result<(String, Option<u8>, Option<u64>, Option<u64>, usize, usize, bool, bool), String> {
    let mut fen = STARTPOS_FEN.to_owned(); let mut depth = None; let mut movetime = None; let mut nodes = None; let mut multipv = 1; let mut hash = 16; let mut divide = false; let mut debug = false; let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--fen" => fen = value(args, &mut i, "--fen")?.to_owned(),
            "--depth" => depth = Some(value(args, &mut i, "--depth")?.parse().map_err(|_| "--depth must be an integer from 0 to 255".to_owned())?),
            "--movetime" => movetime = Some(value(args, &mut i, "--movetime")?.parse().map_err(|_| "--movetime must be milliseconds".to_owned())?),
            "--nodes" => nodes = Some(value(args, &mut i, "--nodes")?.parse().map_err(|_| "--nodes must be a non-negative integer".to_owned())?),
            "--multipv" => multipv = value(args, &mut i, "--multipv")?.parse::<usize>().map_err(|_| "--multipv must be an integer".to_owned())?.clamp(1, 5),
            "--hash" => hash = value(args, &mut i, "--hash")?.parse::<usize>().map_err(|_| "--hash must be an integer in MiB".to_owned())?.clamp(1, 4096),
            "--divide" => divide = true,
            "--debug" => debug = true,
            other => return Err(format!("unknown argument `{other}`")),
        }
        i += 1;
    }
    Ok((fen, depth, movetime, nodes, multipv, hash, divide, debug))
}
fn main() -> ExitCode {
    let mut argv = env::args().skip(1).collect::<Vec<_>>();
    if argv.is_empty() || matches!(argv[0].as_str(), "-h" | "--help") { usage(); return ExitCode::SUCCESS; }
    let command = argv.remove(0); let (fen, depth, movetime, nodes, multipv, hash, divide, debug) = match parse_args(&argv) { Ok(v) => v, Err(e) => { eprintln!("error: {e}"); usage(); return ExitCode::from(2); } };
    let mut position = match Position::from_fen(&fen) { Ok(p) => p, Err(e) => { eprintln!("error: {e}"); return ExitCode::from(2); } };
    match command.as_str() {
        "perft" => {
            let d = match depth { Some(v) => v, None => { eprintln!("error: perft requires --depth N"); return ExitCode::from(2); } };
            if divide { for (mv, n) in perft::divide(&mut position, d) { println!("{}: {}", mv.to_uci(), n); } } else { println!("nodes {}", perft::perft(&mut position, d)); }
        }
        "analyze" => {
            let mut engine = Engine::new(hash); let limits = SearchLimits { depth: if depth.is_none() && movetime.is_none() && nodes.is_none() { Some(8) } else { depth }, movetime: movetime.map(Duration::from_millis), nodes, multipv };
            let result = engine.search(&mut position, limits, None);
            println!("Position: {}", position.to_fen()); println!("Depth {}, MultiPV {}", result.depth, multipv);
            for line in &result.lines {
                let mut replay = position.clone(); let san = line.pv.iter().filter_map(|&m| { let text = moves::to_san(&replay, m).ok()?; let _ = replay.try_make_move(m).ok()?; Some(text) }).collect::<Vec<_>>().join(" ");
                println!("{}. score {}  depth {}  pv {}{}", line.rank, chess_engine::search::format_score(line.score), line.depth, line.pv.iter().map(|m| m.to_uci()).collect::<Vec<_>>().join(" "), if san.is_empty() { String::new() } else { format!("  san {san}") });
            }
            let best = result.best_move().map(|m| m.to_uci()).unwrap_or_else(|| "0000".into()); println!("bestmove {best}"); println!("nodes {} nps {} time {} hashfull {}", result.nodes, result.nps, result.elapsed.as_millis(), result.hashfull);
            if debug { println!("debug aborted {} tt_hits {} seldepth {}", result.aborted, result.tt_hits, result.seldepth); }
        }
        _ => { eprintln!("error: unknown command `{command}`"); usage(); return ExitCode::from(2); }
    }
    ExitCode::SUCCESS
}
