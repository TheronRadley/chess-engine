# Correctness First Rust Chess Engine

A single-threaded UCI chess engine written in Rust. It accepts strict FEN, produces only legal orthodox-chess moves, and searches with iterative deepening, alpha-beta/PVS, quiescence, a transposition table, and MultiPV.

## Why Rust

The engine is Rust rather than a prototype language because search depth is directly tied to native-code speed: optimized Rust is close to C++ for bitboard-heavy code. At the same time, ownership and bounds checking remove several serious engine failure modes common in C/C++—stale recursive board state, aliased hash-table storage, and array-indexing mistakes. Cargo provides a standard build/test/benchmark workflow without an extra build system. The representation and attack tables are ordinary Rust data, so future magic/PEXT, SIMD, NNUE, and parallel-search work has a natural path forward.

## Design overview

* **Position:** twelve `u64` piece bitboards, white/black/all occupancy bitboards, and a synchronized 64-square mailbox.
* **State:** side, four castling-right bits, en-passant target, clocks, deterministic incremental Zobrist key, and full known-position history.
* **Attacks:** precomputed pawn/knight/king tables plus precomputed occupancy-indexed sliding tables. The latter is a portable software-PEXT table lookup, an established magic-bitboard-equivalent technique without a BMI2 runtime requirement.
* **Move generation:** pseudo-legal candidates are make/unmade against the king attack invariant, so the public list is strictly legal—including pins, double check, discovered attacks, castling transit, and en passant discovered checks. A deliberately independent copy-make reference generator is exposed for test cross-checking. The make/unmake filter was selected for its auditable correctness; pin/check masks are a documented optimization opportunity once profiling shows it is needed.
* **Search:** iterative deepening with aspiration windows, PVS, quiescence, TT bounds/mate-score normalization, MVV-LVA plus legal SEE at upper nodes, killer/history ordering, guarded null move, LMR, shallow futility pruning, and check extensions.
* **Evaluation:** tapered classical material/PST, pawn structure and passers, king safety, mobility, rook files, bishop pair, space, and endgame king activity. Constants are in `src/eval/params.rs`.

All implementation parameters that are not chess rules are collected in `src/eval/params.rs`, `src/search/mod.rs`, and `DECISIONS.md`.

## Build

```sh
cargo build --release
cargo test
cargo test --release                 # includes the 119,060,324-node perft(6)
cargo bench
```

The release profile enables Thin LTO, one codegen unit, and aborting panics for the engine binaries. On a clean environment Cargo/Rust (edition 2021 compatible) is the only prerequisite—there are no third-party crate dependencies.

> **Verification note:** This Arena sandbox has no local `rustc`/`cargo`, so local execution is unavailable. The complete release build, tests (including start-position perft(6)), CLI/UCI smoke checks, and benchmarks were instead run in the repository's GitHub Actions Ubuntu runner. The successful captured run is [Rust CI #36160655147](https://github.com/TheronRadley/chess-engine/actions/runs/36160655147). The real outputs below are copied from that run, not invented examples.

## CLI

The `chess-engine` binary is a direct analysis/perft interface over the same library used by UCI:

```sh
cargo run --release --bin chess-engine -- analyze \
  --fen "rnbqkbnr/pppppppp/8/8/4P3/8/PPPP1PPP/RNBQKBNR b KQkq e3 0 1" \
  --depth 8 --multipv 3 --hash 64

cargo run --release --bin chess-engine -- perft --fen "r3k2r/p1ppqpb1/bn2pnp1/3PN3/1p2P3/2N2Q1p/PPPBBPPP/R3K2R w KQkq - 0 1" --depth 4
```

`analyze` supports `--fen`, `--depth`, `--movetime` (milliseconds), `--nodes`, `--multipv` (clamped 1–5), `--hash` (MiB, 1–4096), and `--debug`. It prints UCI PV moves and SAN alongside them, a centipawn or mate score, depth, nodes, NPS, elapsed milliseconds, and UCI-style `hashfull` permille. `--debug` additionally reports cancellation state, TT hits, and selective depth. `perft --divide` prints each root move's count.

Actual output captured from the successful release CI run:

```text
$ target/release/chess-engine analyze --fen "7k/5K2/6Q1/8/8/8/8/8 w - - 0 1" --depth 3 --multipv 1 --hash 4
Position: 7k/5K2/6Q1/8/8/8/8/8 w - - 0 1
Depth 3, MultiPV 1
1. score mate 1  depth 3  pv g6h5  san Qh5#
bestmove g6h5
nodes 142 nps 1014778 time 0 hashfull 0

$ target/release/chess-engine perft --depth 3
nodes 8902
```

## UCI

Run:

```sh
cargo run --release --bin chess-uci
```

Implemented commands are `uci`, `isready`, `ucinewgame`, `position startpos [moves ...]`, `position fen <six fields> [moves ...]`, `go depth|movetime|nodes|wtime|btime|winc|binc|infinite`, `stop`, `quit`, and `setoption` for `Hash`, `MultiPV`, and `Threads`.

`Threads` is deliberately advertised with min=max=1: the engine is safely single-threaded in this release. This is a real transcript captured by release CI:

```text
uci
id name Correctness First Rust Chess
id author TheronRadley contributors
option name Hash type spin default 16 min 1 max 4096
option name MultiPV type spin default 1 min 1 max 5
option name Threads type spin default 1 min 1 max 1
uciok
isready
readyok
position fen 7k/5K2/6Q1/8/8/8/8/8 w - - 0 1
go depth 3
info string searching
info depth 3 seldepth 2 multipv 1 score mate 1 nodes 142 nps 905346 hashfull 0 time 0 pv g6h5
bestmove g6h5
```

Search runs on a worker thread so the protocol reader can process `stop`; it returns the last completed iteration (or a deterministic legal fallback if stopped before depth one).

## Validation

`tests/perft_tests.rs` contains the canonical start position through depth 6, Kiwipete, positions 3–5, promotion, en-passant, and production/reference-generator comparisons. The six-ply start-position test is ignored only in debug builds because it is 119M nodes; it runs in release validation. GitHub Actions runs `cargo build --release --all-targets` and the complete `cargo test --release` suite on every branch push; run #36160655147 passed.

`tests/rules_tests.rs` covers FEN strictness and round trips, movement/blocking, mate/stalemate, pin/discovery, castling conditions, legal/illegal en passant, draw rules, SAN/UCI, and repetition. `tests/search_tests.rs` covers mate scoring/PV replay, MultiPV ordering and uniqueness, time-budget overshoot, and TT mate normalization. The internal UCI layer rejects malformed protocol position/move text as `info string error` rather than guessing.

## Benchmarks

The repository has dependency-free `cargo bench` harnesses:

* `perft_bench`: start position perft depth 5 and raw nodes/sec.
* `search_bench`: a forced-mate tactical position at depth 12 and search nodes/sec.

Actual `cargo bench` output from GitHub Actions Ubuntu (`ubuntu-latest`, Rust stable, run #36160655147) was:

```text
perft startpos depth 5: 4865609 nodes in 199.365687ms (24405448 nps)
search tactical mate depth 12: 89381 nodes in 48.578587ms (1841146 nps), best Some("g6h5")
```

Hosted runner CPU allocation can vary, so these are a reproducible baseline for the recorded environment rather than a cross-machine performance guarantee.

## Known limitations and extension points

* No NNUE, Syzygy/tablebases, opening book, or contempt. Insufficient-material detection intentionally handles material-decidable K/K, K+B/K, K+N/K, and same-colour K+B/K+B only; see code comments.
* No multithreading yet. `Engine` owns mutable search heuristics and TT, deliberately avoiding globals so Lazy SMP can add worker-local state later.
* The portable slider table index is software PEXT. Native BMI2/PEXT or generated magic multipliers are a local replacement in `movegen` if profiling warrants it.
* Legal generation favours a shared make/unmake king-safety invariant over direct check/pin masks. It is strictly legal and test-oracle cross-checked, but direct pin-mask generation is the most valuable remaining move-generation optimization.
* The hand-tuned evaluation is intentionally classical and transparent; tuning and NNUE are future work, not silently approximated.
