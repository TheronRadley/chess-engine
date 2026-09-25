# Engineering decisions

## Determinism

Zobrist keys come from a fixed SplitMix64 seed, not process entropy. Given a position history, configuration, table contents, and limits, move ordering has deterministic tie breaking by encoded move. This is useful for regressions and avoids the common confusion of random keys changing a test result.

## Position mutation

Search uses one mutable `Position` plus compact `Undo` records, not a board clone per node. `Undo` restores captured piece/square, castling rights, en-passant square, clocks, key, and history length. The mailbox and all bitboards are changed only by paired add/remove helpers. The independent reference generator intentionally does use copy-make, but only in tests.

## Legal generation before optimization

The production generator enumerates piece-legal geometry/castling transit conditions and then checks the moving king after make/unmake. This is slower than specialized check-evasion/pin masks but gives one authoritative legality condition for normal moves, pins, double checks, and en passant discovery. It is not a placeholder: all returned moves are strict legal moves and canonical perft/reference comparison protects it. Direct pin-mask/checker generation remains a measured future optimization, not a speculative rewrite.

## Slider lookup

Sliding attacks are precomputed for every relevant occupancy subset and indexed by a portable software PEXT compression. This gives a constant table lookup attack result without magic-number generation or CPU feature dispatch. The table layout is an equivalent established sliding-bitboard technique; a BMI2 or magic index can replace only the compression function later.

## Hashing and repetition

The Zobrist key incrementally XORs pieces, side, the 16-combination castling key, and EP file. The position history includes the current key and is retained when UCI applies a move list. The FEN EP target is validated and included exactly as supplied/created by a double push. Fifty moves uses the FIDE 100-halfmove threshold.

## Draw material scope

Automatically dead material is limited to K/K, a lone bishop/knight versus king, and two bishops (one per side) on same-colour squares. This conservative rule avoids incorrectly claiming a draw in unusual multi-minor positions whose forced-mate status is non-obvious.

## Search choices and tunables

* Default TT: **16 MiB**, configurable 1–4096 MiB. One entry per index, depth-preferred with generation replacement.
* Aspiration half-window: **40 cp**. It doubles on a fail high/low.
* Null move: non-PV, not in check, depth >= **3**, and only with non-pawn material. Reduction is `2 + depth / 6`; pawn-only sides are excluded for zugzwang safety. A null fail-high is additionally verified at reduced depth with null moves disabled.
* LMR: quiet non-check moves from index 4 at depth >= **3**, reduction `1 + index / 12`; a fail-high gets full-depth re-search.
* Futility: non-PV quiets at depth <= **2** when static eval + `95 * depth` cannot reach alpha.
* Quiescence searches legal captures/promotions, and all evasions while checked. Delta pruning uses a deliberately generous **975 cp** queen swing.
* Check extension: +1 ply whenever the side to move is checked.
* MultiPV: every root move is full-window searched at each iteration, lines are score-sorted, and the requested top 1–5 roots are retained. It is slower than a single root PVS but prevents bound-only second/third lines and keeps MultiPV semantically simple.

The move-order priority is TT move, capture MVV-LVA/SEE classification, killer moves (two slots per ply), history, then deterministic UCI encoding. Exact legal SEE is used for captures in the upper two plies where it is valuable; deeper nodes retain MVV-LVA classification to avoid exchange-tree overhead.

## Time and interruption

A `movetime` is turned into a hard deadline two milliseconds early. Search checks it (and an atomic UCI stop flag) at least every 2048 nodes and at root. Iterative deepening only commits a fully completed depth; on interruption it returns that depth, with a legal fallback when no depth completes. Clock-based UCI allocation uses `remaining/30 + 3/4 increment`, capped below remaining clock, as a conservative single-threaded baseline.

## Evaluation

The values/PST/term weights in `src/eval/params.rs` are intentionally modest starting values, not alleged Elo tuning. Evaluation is recomputed at leaves to keep state handling auditable. Incremental material/PST cache is a candidate optimization after profiler evidence; no correctness-sensitive cached score is maintained today.
