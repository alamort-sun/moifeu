# Orb — a little procedural life

A playable local orb-weaver pet. The CPU simulation is Rust compiled to WebAssembly; the browser receives a render snapshot and draws it on Canvas. No package installation, accounts, model credits, external assets, or network services are needed to play.

## Open on this Mac

Double-click **Start Orb.command**. The included Apple Silicon executable opens a local habitat in your default browser. Keep its terminal window open while playing; Control-C stops it. Each launch chooses an available loopback port. Browser refresh starts a new pet; state is not saved between sessions.

- **Wander:** explore a bounded habitat.
- **Follow:** follow your pointer, or tap a destination on a touch screen.
- **Rest:** settle the body and draw the feet inward.
- **Weave:** progressively reveal radial and spiral silk, using a deformable constraint graph. The spider stays at the weaving location; this version does not navigate strand by strand.
- Tap near the spider for a small greeting.
- **Habitat:** adjust curiosity and alertness, enable rounded stepping stones, or inspect joint/contact markers.
- **Pause**, **Clear silk**, and **Start anew** are available. Keyboard: 1–4 select behaviors; Space pauses when a control does not have focus. Reduced-motion preference starts the pet paused at rest.

## Architecture

The earlier Moifeu files in the parent directory are retained as a protocol draft. This independent Cargo crate is the executable implementation; it does not depend on the incomplete parent package or its beam transport.

| Module | Responsibility |
|---|---|
| `math` | Dependency-free three-dimensional vector operations |
| `physics` | Damped Verlet particles, inverse-mass weighted distance constraints, planted-foot tension |
| `body` | Thorax and abdomen rings, cross-braces, eight particle attachments |
| `ik` | Generic iterative FABRIK, including unreachable targets |
| `world` | TerrainSampler trait; flat or rounded heightfield, surface normals |
| `locomotion` | Terrain-aware desired foot placement, alternating four-leg gait, smooth lifted steps |
| `behavior` | Desired motion and personality-to-parameter mapping |
| `silk` | Radial/spiral graph; pinned boundary nodes and the shared constraint solver |
| `visual` | Curved color trajectory between cool and warm anchor palettes |
| `lib` | Fixed-step coordination, second IK pass, flat render snapshot, WASM exports |
| `web/` | Responsive canvas renderer, pointer and accessible button controls |
| `src/bin/serve.rs` | Dependency-free loopback-only asset server |

The body has 22 particles; the spider has eight independent four-joint chains. Gait only requests steps. At most one four-leg group moves. Planted world targets remain unchanged between steps; extension pulls on the corresponding body particles before integration. Body motors apply forces toward a desired pose; body positions are not simply replaced by render coordinates. After constraint solving, a second IK pass updates legs against the moved hips.

The simulation runs at 120 Hz with a bounded frame accumulator. Browser rendering cannot write particle or joint state. App inputs reject nonfinite coordinates. The WASM snapshot begins with version 1, then a 16-float header, body triples, eight 14-float leg records, and six floats per visible silk edge. The JavaScript view reacquires WASM memory after every update to accommodate memory growth.

This is a stylized 2.5D pet, not a biomechanical or rigid-body dynamics model. GPU compute is intentionally deferred, as allowed by the brief; a single pet and 195 silk constraints run on the CPU. No desktop-overlay, Pantheon message transport, or native Swift shell is installed.

## Rebuild

Requires a stable Rust toolchain managed by rustup and the `wasm32-unknown-unknown` target. Run `./build.sh`. There are no Cargo dependencies. It runs native tests, builds the WASM library, and builds the local server. The packaged Mac binary is for Apple Silicon; rebuilding produces the server for your current host.

## Verification

Six Rust tests cover fixed-particle constraints, reachable/unreachable FABRIK and segment lengths, tension direction, two minutes of turning/walking over terrain with planted-contact error bounded to 0.02 world units and four support feet, web completion/stability, and deterministic fixed-step partitioning. A separate WASM smoke test exercises the actual browser binary for walking, terrain, support count, and full web completion. Browser checks cover rendering, controls, web completion, narrow layout and console errors.
