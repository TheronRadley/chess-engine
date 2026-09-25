use chess_engine::{perft, Position};

fn count(fen: &str, depth: u8) -> u64 { let mut p = Position::from_fen(fen).expect("reference FEN"); perft::perft(&mut p, depth) }

#[test]
fn start_position_perft() {
    let fen = "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1";
    assert_eq!(count(fen, 1), 20); assert_eq!(count(fen, 2), 400); assert_eq!(count(fen, 3), 8902); assert_eq!(count(fen, 4), 197_281); assert_eq!(count(fen, 5), 4_865_609);
}

/// This exact required reference count is intentionally opt-in in debug builds:
/// pseudo-legal-plus-filter is a correctness oracle-style implementation and
/// 119M debug nodes is not CI-friendly. It runs automatically in release test
/// jobs (`cargo test --release`) and can always be requested with this test.
#[test]
#[cfg_attr(debug_assertions, ignore = "run with cargo test --release -- --ignored for the 119M-node reference")]
fn start_position_perft_six() { assert_eq!(count("rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1", 6), 119_060_324); }

#[test]
fn canonical_perft_positions() {
    let kiwipete = "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1";
    assert_eq!(count(kiwipete, 1), 48); assert_eq!(count(kiwipete, 2), 2039); assert_eq!(count(kiwipete, 3), 97_862);
    let p3 = "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1";
    assert_eq!(count(p3, 1), 14); assert_eq!(count(p3, 2), 191); assert_eq!(count(p3, 3), 2812); assert_eq!(count(p3, 4), 43_238);
    let p4 = "r3k2r/Pppp1ppp/1b3nbN/nP6/BBP1P3/q4N2/Pp1P2PP/R2Q1RK1 w kq - 0 1";
    assert_eq!(count(p4, 1), 6); assert_eq!(count(p4, 2), 264); assert_eq!(count(p4, 3), 9467);
    let p5 = "rnbq1k1r/pp1Pbppp/2p5/8/2B5/8/PPP1NnPP/RNBQK2R w KQ - 0 1";
    assert_eq!(count(p5, 1), 44); assert_eq!(count(p5, 2), 1486); assert_eq!(count(p5, 3), 62_379);
}

#[test]
fn production_and_copy_make_oracle_agree() {
    for fen in [
        "rnbqkbnr/pppppppp/8/8/8/8/PPPPPPPP/RNBQKBNR w KQkq - 0 1",
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "8/2p5/3p4/KP5r/1R3p1k/8/4P1P1/8 w - - 0 1",
    ] {
        let p = Position::from_fen(fen).unwrap(); let a = p.legal_moves(); let b = chess_engine::movegen::reference_legal_moves(&p);
        let mut av = a.iter().map(|m| m.to_uci()).collect::<Vec<_>>(); let mut bv = b.iter().map(|m| m.to_uci()).collect::<Vec<_>>(); av.sort(); bv.sort(); assert_eq!(av, bv, "{fen}");
    }
}

#[test]
fn promotion_and_en_passant_are_present() {
    let p = Position::from_fen("7k/P7/8/8/8/8/7p/K7 w - - 0 1").unwrap(); let promotions = p.legal_moves().iter().filter(|m| m.from() == 48 && m.to() == 56).count(); assert_eq!(promotions, 4);
    let p = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap(); assert!(p.legal_moves().iter().any(|m| m.to_uci() == "e5d6"));
}
