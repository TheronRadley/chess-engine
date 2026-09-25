use chess_engine::{moves, Color, PieceKind, Position, STARTPOS_FEN};

#[test]
fn fen_round_trips_and_rejects_bad_metadata() {
    for fen in [
        STARTPOS_FEN,
        "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1",
        "4k3/8/8/8/8/8/4K3/8 b - - 99 42",
        "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1",
    ] { assert_eq!(Position::from_fen(fen).unwrap().to_fen(), fen); }
    assert!(Position::from_fen("8/8/8/8/8/8/8/8 w - - 0 1").is_err());
    assert!(Position::from_fen("4k3/8/8/8/8/8/4K3/8 w K - 0 1").is_err());
    assert!(Position::from_fen("4k3/8/8/8/8/8/4K3/8 w - e3 0 1").is_err());
}

#[test]
fn all_piece_motion_and_blocking() {
    let p = Position::from_fen("4k3/8/8/8/3Q4/8/4K3/8 w - - 0 1").unwrap(); let moves = p.legal_moves();
    assert!(moves.iter().any(|m| m.to_uci() == "d4h8")); assert!(moves.iter().any(|m| m.to_uci() == "d4d8")); assert!(moves.iter().any(|m| m.to_uci() == "d4a1"));
    let p = Position::from_fen("4k3/8/8/3p4/3Q4/8/4K3/8 w - - 0 1").unwrap(); assert!(!p.legal_moves().iter().any(|m| m.to_uci() == "d4d8"));
    let p = Position::from_fen("4k3/8/8/8/8/8/3N4/4K3 w - - 0 1").unwrap(); assert_eq!(p.legal_moves().iter().filter(|m| m.from() == 11).count(), 6);
}

#[test]
fn check_mate_stalemate_pins_and_discovery() {
    let mate = Position::from_fen("7k/6Q1/6K1/8/8/8/8/8 b - - 0 1").unwrap(); assert!(mate.in_check(Color::Black)); assert!(mate.is_checkmate());
    let stale = Position::from_fen("7k/5Q2/6K1/8/8/8/8/8 b - - 0 1").unwrap(); assert!(!stale.in_check(Color::Black)); assert!(stale.is_stalemate());
    let pin = Position::from_fen("4r1k1/8/8/8/8/8/4R3/4K3 w - - 0 1").unwrap(); assert!(!pin.legal_moves().iter().any(|m| m.to_uci() == "e2d2")); assert!(pin.legal_moves().iter().any(|m| m.to_uci() == "e2e8"));
    let mut disc = Position::from_fen("4k3/8/8/8/8/8/4B3/4R1K1 w - - 0 1").unwrap(); let mv = moves::parse_uci_move(&disc, "e2b5").unwrap(); let _ = disc.try_make_move(mv).unwrap(); assert!(disc.in_check(Color::Black));
}

#[test]
fn castling_and_en_passant_edge_cases() {
    let p = Position::from_fen("r3k2r/8/8/8/8/8/8/R3K2R w KQkq - 0 1").unwrap(); let m = p.legal_moves(); assert!(m.iter().any(|x| x.to_uci() == "e1g1")); assert!(m.iter().any(|x| x.to_uci() == "e1c1"));
    let p = Position::from_fen("r3k2r/8/8/8/8/8/5r2/R3K2R w KQkq - 0 1").unwrap(); assert!(!p.legal_moves().iter().any(|x| x.to_uci() == "e1g1"));
    // e5xd6 e.p. would open e-rank? Here it is legal and removes d5.
    let mut p = Position::from_fen("4k3/8/8/3pP3/8/8/8/4K3 w - d6 0 1").unwrap(); let mv = moves::parse_uci_move(&p, "e5d6").unwrap(); let _ = p.try_make_move(mv).unwrap(); assert_eq!(p.piece_at(35), None); assert_eq!(p.piece_at(43).unwrap().kind, PieceKind::Pawn);
    // The classic illegal EP horizontal discovery: e5xd6 would expose e1 to a5 rook.
    let p = Position::from_fen("4k3/8/8/r4PpK/8/8/8/8 w - g6 0 1").unwrap(); assert!(!p.legal_moves().iter().any(|m| m.to_uci() == "f5g6"));
}

#[test]
fn draw_rules_and_notation() {
    let p = Position::from_fen("4k3/8/8/8/8/8/4K3/8 w - - 100 1").unwrap(); assert!(p.is_fifty_move_draw()); assert!(p.is_insufficient_material());
    let p = Position::from_fen("4k3/8/8/8/8/8/3BK3/8 w - - 0 1").unwrap(); assert!(p.is_insufficient_material());
    let p = Position::from_fen("4kb2/8/8/8/8/8/3B4/4K3 w - - 0 1").unwrap(); assert!(p.is_insufficient_material());
    let p = Position::from_fen("4k3/8/8/8/8/8/3BK3/8 w - - 0 1").unwrap(); let mv = moves::parse_uci_move(&p, "d2e3").unwrap(); assert_eq!(moves::to_san(&p, mv).unwrap(), "Be3");
    let p = Position::from_fen("4k3/8/8/8/8/8/1N1NK3/8 w - - 0 1").unwrap(); let mv = moves::parse_uci_move(&p, "b2c4").unwrap(); assert_eq!(moves::to_san(&p, mv).unwrap(), "Nbc4");
}

#[test]
fn threefold_tracks_full_game_history() {
    let mut p = Position::from_fen("4k1n1/8/8/8/8/8/8/4K1N1 w - - 0 1").unwrap();
    for text in ["g1f3", "g8f6", "f3g1", "f6g8", "g1f3", "g8f6", "f3g1", "f6g8"] { let mv = moves::parse_uci_move(&p, text).unwrap(); let _ = p.try_make_move(mv).unwrap(); }
    assert!(p.is_threefold_repetition());
}
