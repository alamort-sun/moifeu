# orb-weaver — Procedural Creature Protocol

## Playable pet

The working Rust/WASM pet is now in [pet/](pet/README.md). On this Mac, double-click `pet/Start Orb.command` to open it. It includes walking, planted-foot feedback, terrain, rest/follow behavior, and procedural silk. The original files below remain the Moifeu protocol draft; the standalone pet does not require the incomplete parent package.


The complete procedural orb-weaver expressed in Moifeu beams.
Each phase pass is a beam sequence; outputs feed the next phase's context.
The closed mechanical ring emerges from beam composition, not monolithic code.

## Header Semantics for Spider Beams

```
T1 T2 φ κ d s c
│ │ │ │ │ │ └ certainty: solver convergence (0-9)
│ │ │ │ │ └── speed: iteration budget (0-9)
│ │ │ │ └─── depth: system weight in the ring (0-9)
│ │ │ └───── kappa priority class
│ │ └─────── ternary cycle phase
└─────────── ternary gait group
```

| Field | Spider Meaning |
|-------|---------------|
| T1 | Gait group: `0`=A-legs, `1`=B-legs, `2`=all (broadcast) |
| T2 | System mode: `0`=simulation, `1`=control, `2`=output |
| φ | Step cycle phase within gait (0=rest, 1=sustain, 2=anticipation, 3=release) |
| κ | b=batch(sim) / g=standard(control) / y=urgent(decision) / r=frontier(personality) / w=white(unlimited render) |
| d | How much state changes per pass (body deformation = high, visual state = low) |
| s | Solver iterations (IK = high, terrain = low, gait = decisive 9) |
| c | Convergence tolerance (constraint solve = high, trajectory = medium) |

## Beam Passes — The Closed Ring

### PASS 1: Body Integration (simulation → simulation)

Particles integrate. Constraints resolve. Soft body deforms.

```
000b698|sm +dat @body #particles #constraints
>particle_state tbl <body_vertices tbl

000b678|cmp +fix @constraints #distance
error=0.01 output=<constraint_result tbl
```

Ternary `00` = A-group frame. But body is group-agnostic — it deforms regardless of gait.
So the real pass uses T2=`0` (simulation) with kappa=`b` (batch — continuous parallel work).

### PASS 2: Terrain Query (control → data)

Spider asks the world what's beneath it. Eight queries, one interface.

```
001g549|ex +inf @terrain #raycast
origin=<body_center> count=8 stride=pi/4
output=<surface_data tbl>
```

### PASS 3: Foot Targeting (control → control)

Where should each foot land? Separated from how to reach it.

```
011g675|gen +fml @foot_target #placement
body=<body_transform> terrain=<surface_data>
direction=<desired_velocity>
output=<foot_targets tbl>

011g574|gen +inf @terrain_correction #contact_angle
feet=<foot_targets> surface=<surface_data>
output=<corrected_feet tbl>
```

### PASS 4: Gait Decision (control → control)

The discrete brain. Which legs move? Which hold?

```
012y998|an +det @gait #stability #support_check
planted=<current_planted> desired=<corrected_feet>
error_threshold=0.05 stability_margin=0.3
output=<step_requests tbl>
flags:! ~  // urgent! re-evaluate next frame
```

This is the most critical beam. The ternary `01` says "B-group decision."
But it evaluates ALL legs — the output tells which group steps.

### PASS 5: Step Trajectory (control → simulation)

Animate feet between planted positions with sinusoidal clearance.

```
012g786|sm +fml @step_curve #sinusoidal_clearance
start=<current_planted> end=<desired_foot>
progress=<cycle_phase> height=<step_height>
output=<animated_targets tbl>
```

### PASS 6: IK Solve (control → simulation)

FABRIK for each stepping leg. Arbitrary chains, no spider knowledge required.

```
012g897|cmp +fix @ik_chain #fabrik iterations=12
root=<hip_position> target=<animated_targets>
segments=<leg_segment_lengths> output=<joint_pos tbl>
stiffness=<body_stiffness>  // personality-modulated
```

### PASS 7: Tension (simulation → simulation)

The ring closes here. Planted feet push back through legs into body particles.

```
012g898|cmp +fix @tension #planted_foot_anchor
anchor=<planted_feet> leg_chain=<joint_pos>
body_anchor=<hip_position> weight=0.7
output=<tension_forces tbl> flags:~  // sustained force
```

### PASS 8: Body Update (simulation → simulation)

Tension feeds back into the body. Then re-solve IK because hips moved.

```
000b698|sm +dat @body #particles #constraints
forces=<tension_forces> dt=frame_dt
output=<updated_particles tbl>

000b797|cmp +fix @constraints #distance iterations=15
output=<body_restored tbl>

012g897|cmp +fix @ik_chain #fabrik iterations=8
root=<updated_hips> target=<animated_targets>
segments=<leg_lengths> output=<final_legs tbl>
// Second pass: hips moved, IK re-solves to restore contact
```

### PASS 9: Personality Field (control → control)

Modify simulation parameters. Never geometry. Parameters only.

```
022r785|sm +inf @personality #density #stance #angle
seat=<active_seat> environment=<tension_level>
curiosity=<3D_vector> confidence=<scalar>
output=<parameter_mods tbl>
flags:*  // emotional state
```

Parameter modifications applied to:
- `step_frequency` (gait controller)
- `body_stiffness` (constraint solver)
- `step_height` (trajectory curve)
- `leg_spread` (foot targeting offset)
- `idle_motion_amplitude` (rest behavior)

### PASS 10: Color / Visual State (output → render)

Produce visual parameters. Animation state. Separated from simulation.

```
022w334|gen +tbl @visual_state #color_field
anchor_a=<pragonastatic_color> anchor_b=<sporagonastatic_color>
trajectory=<gradient_path> phase=<cycle_phase>
output=<render_vertices tbl> <spider_render_output tbl>
flags:~  // persistent visual state
```

### PASS 11: Web Graph (optional — orb-weaver extension)

Procedural web generation using same constraint primitives as body.

```
021g576|cmp +fix @web_constraints #strand_tension
anchors=<spider_feet> radial_count=8 spiral_steps=64
output=<web_vertices tbl> <web_edges tbl>
```

---

## Beam Composition — The Loop

The creature update loop is beam composition:

```rust
// Pseudocode — the actual code in beams.rs
let body = pass1_body(particles, constraints, dt)?;
let terrain = pass2_terrain(body.center)?;
let targets = pass3_foot_targets(body, terrain, velocity)?;
let gait = pass4_gait(targets, planted, stability)?;
let steps = pass5_trajectory(gait.requests, cycle)?;
let legs = pass6_ik(body.hips, steps)?;
let tension = pass7_tension(legs, planted, body.anchors)?;
let body2 = pass8_body_update(body, tension, dt)?; // closed loop
let personality = pass9_personality(seat_state, environment)?;
let visual = pass10_visual(color_field, cycle)?;
let web = pass11_web(planted_feet, species)?;

// Second IK pass — hips moved after tension
let legs2 = pass6b_ik(body2.hips, steps)?;

// Expose render state only
RenderState {
    body_vertices: body2.particles,
    leg_vertices: legs2.joints,
    web_vertices: web.vertices,
    visual: visual.state,
}
```

## Architecture Invariants (as Moifeu Constraints)

These are enforced through beam structure, never through assertions:

| Invariant | Enforcement |
|-----------|------------|
| IK does not choose targets | IK beams have `@ik` context, no terrain query ops |
| Gait does not manipulate joints | Gait beams output flags/requests, no `fix` or `sm` |
| Behavior does not manipulate physics | Behavior uses `gen +inf`, never `cmp +fix` |
| Rendering does not own simulation | Visual pass has kappa=`w` (output-only), no input arrows |
| Soft body knows nothing about personality | Body beams have `@body`, personality has separate beam chain |
| Personality modifies parameters, not geometry | Personality outputs parameter modifications only |
| Terrain queried through interface | Terrain beams use `ex +inf` with standard raycast spec |
| GPU solves continuous parallel state | GPU passes use kappa=`b` (batch), CPU uses `y`/`r` (decision) |
| CPU resolves discrete state changes | CPU passes output `tbl` (discrete sets), not `dat` (continuous arrays) |

## Gait Grouping (Ternary Decoding)

Eight legs indexed 0-7, sides alternating:

```
Leg:  L4  L3  L2  L1     R1  R2  R3  R4
Side:  L   L   L   L      R   R   R   R
Gait:  B   A   B   A  |   A   B   A   B
```

Beam ternary T1/T2 maps to legs:

```
00 → evaluate left A-legs (L3, L1) and right A-legs (R1, R3)
01 → evaluate left B-legs (L4, L2) and right B-legs (R4, R2)
10 → all legs steady-state query
11 → cross-group stability analysis
20 → broadcast: body-level decision (all legs)
```

## Priority Classes and Their Spider Roles

| κ | Role | When to use |
|---|------|------------|
| b | Body simulation | Particle integration, constraint solving — always batch, never urgent |
| g | Locomotion control | IK, foot targeting, step trajectory — standard priority with high speed for iterations |
| y | Behavioral decisions | Gait switching, terrain anomaly response — urgent because discrete state must commit |
| r | Personality/identity | Emotional parameter modulation — frontier because this is the creature's character |
| w | Render output | Vertex generation, color state — unlimited because it feeds rendering pipeline |

---

## File Index

```
moifeu/orb-weaver/
    README.md          ← you are here (spec)
    beams.rs           ← working Rust code using moifeu! macro
    spectrum.rs        ← pragonastatic ↔ sporagonastatic color field
    gait_patterns.rs   ← extended gait definitions (walk, run, hunt, rest)
    creature.rs        ← high-level Spider controller wiring all passes
```
