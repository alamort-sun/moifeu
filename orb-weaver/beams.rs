// Procedural Orb-Weaver — Moifeu Beam Definitions
// Each pub fn produces a valid MoifeuBeam wire string via emit_beam().
// The beams implement the full closed mechanical ring.
//
// Integration: This file lives under alamort/moifeu/orb-weaver/
// When mounted in core/lib.rs as `#[path] pub mod creature`, the moifeu
// types come through `crate::moifeu::` (the parent module).
use crate::moifeu::{emit_beam, parse_beam, MoifeuBeam, MoifeuBeamBuilder};
use crate::moifeu::{emit_beam, parse_beam, MoifeuBeam, MoifeuBeamBuilder};

// ============================================================================
// HEADER CONSTANTS — Spider-specific ternary/kappa encoding
// ============================================================================

/// Gait group selectors (T1)
pub const GAIT_LEFT_A: u8 = 0; // left A-legs + right A-legs
pub const GAIT_LEFT_B: u8 = 0; // left B-legs (ternary context carries side info)
pub const GAIT_RIGHT_A: u8 = 0; // right A-legs
pub const GAIT_RIGHT_B: u8 = 0; // right B-legs
pub const GAIT_ALL: u8 = 2; // broadcast — body-level evaluation

/// System mode (T2)
const SIM_MODE: u8 = 0; // simulation (continuous state)
const CTRL_MODE: u8 = 1; // control (discrete decisions)
const OUT_MODE: u8 = 2; // output (render state)

/// Gait phase hints (φ)
const PHASE_REST: u8 = 0;
const PHASE_SUSTAIN: u8 = 1;
const PHASE_ANTI: u8 = 2; // anticipation — "hold the possibilities"
const PHASE_RELEASE: u8 = 3;

// ============================================================================
// KAPPA PRIORITY FOR SPIDER SYSTEMS
// ============================================================================
const K_BATCH: char = 'b'; // body simulation — batch/continuous
const K_CTRL: char = 'g'; // locomotion control — standard
const K_DECISION: char = 'y'; // behavioral decisions — urgent
const K_PERSONALITY: char = 'r'; // identity/emotional state — frontier
const K_RENDER: char = 'w'; // render output — unlimited

// ============================================================================
// PASS 1: BODY INTEGRATION — Particle + Constraint Simulation
// ============================================================================

/// Integrate soft-body particles via Verlet.
/// Input: particle_state, constraint_list, dt
/// Output: updated_particles
pub fn pass_body_integration(dt: f32) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(SIM_MODE, SIM_MODE, PHASE_SUSTAIN)
        .kappa(K_BATCH)
        .analogue(6, 9, 8) // depth=high for deformation, speed=12 iterations, certainty=8 converges tight
        .context("body_integration")
        .operation("sm") // simulate
        .flag('!') // urgent — body must update every frame
        .input(format!("dt={}", format_float(dt)))
        .constraint("particles")
        .constraint("constraints")
        .modifier("dat") // data output (continuous arrays)
        .build()
        .expect("body_integration beam")
}

/// Distance constraint solver for soft body topology.
/// Iterative — called multiple times per frame.
pub fn pass_constraint_solve(iterations: u8, tolerance: f32) -> MoifeuBeam {
    let speed = std::cmp::min(iterations as u8, 9);
    MoifeuBeamBuilder::new()
        .header(SIM_MODE, SIM_MODE, PHASE_SUSTAIN)
        .kappa(K_BATCH)
        .analogue(6, speed, 9) // high certainty for constraint resolution
        .context("constraint_solve")
        .operation("cmp") // compare/resolve
        .modifier("fix") // fix constraints
        .input(format!("tol={}", format_float(tolerance)))
        .constraint("distance")
        .build()
        .expect("constraint_solve beam")
}

/// Bridge constraints between thorax and abdomen (specific topology).
pub fn pass_thorax_abdomen_bridge(
    thorax_center: &[f32; 3],
    abdomen_center: &[f32; 3],
) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(SIM_MODE, SIM_MODE, PHASE_SUSTAIN)
        .kappa(K_BATCH)
        .analogue(7, 6, 7)
        .context("body_bridge")
        .operation("cmp")
        .modifier("fix")
        .constraint("area") // preserve thorax/abdomen volume separately
        .input(format!(
            "thorax={},{},{} abdomen={},{},{}",
            thorax_center[0],
            thorax_center[1],
            thorax_center[2],
            abdomen_center[0],
            abdomen_center[1],
            abdomen_center[2]
        ))
        .build()
        .expect("thorax_abdomen_bridge beam")
}

// ============================================================================
// PASS 2: TERRAIN QUERY — Surface data beneath spider
// ============================================================================

/// Raycast terrain query from body center and each leg root.
pub fn pass_terrain_query(center_x: f32, center_y: f32, radius: f32) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(SIM_MODE, CTRL_MODE, PHASE_REST)
        .kappa(K_CTRL)
        .analogue(5, 4, 7)
        .context("terrain_query")
        .operation("ex") // execute query
        .modifier("inf") // return full surface info (position, normal, surface_id)
        .input(format!(
            "origin={:.3},{:.3} radius={:.2} count=8 stride=0.7854",
            center_x, center_y, radius
        ))
        .constraint("raycast")
        .build()
        .expect("terrain_query beam")
}

// ============================================================================
// PASS 3: FOOT TARGETING — Where should each foot land?
// ============================================================================

/// Calculate desired foot positions from body state + movement direction.
pub fn pass_foot_targets(
    body_center_x: f32,
    body_center_y: f32,
    body_heading: f32,
    velocity_x: f32,
    velocity_y: f32,
) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_ANTI)
        .kappa(K_CTRL)
        .analogue(6, 7, 5) // moderate certainty — targets are suggestions, not commitments
        .context("foot_target")
        .operation("gen") // generate (procedural placement)
        .modifier("fml") // formal geometric output
        .input(format!(
            "body={:.3},{:.3},{} vel={:.2},{:.2} offset=0.6",
            body_center_x, body_center_y, body_heading, velocity_x, velocity_y
        ))
        .constraint("placement")
        .build()
        .expect("foot_targets beam")
}

/// Terrain correction for foot targets — adjust placement based on surface angle.
pub fn pass_foot_terrain_correction(feet_input: &str) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_ANTI)
        .kappa(K_CTRL)
        .analogue(5, 6, 7)
        .context("foot_correction")
        .operation("gen")
        .modifier("inf")
        .input(feet_input)
        .constraint("contact_angle")
        .build()
        .expect("foot_terrain_correction beam")
}

// ============================================================================
// PASS 4: GAIT CONTROLLER — Which legs move?
// ============================================================================

/// Decide which gait group should step based on foot error + stability.
pub fn pass_gait_decision(
    planted_feet_input: &str,
    desired_feet_input: &str,
    stability_margin: f32,
) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_ANTI)
        .kappa(K_DECISION) // urgent — discrete state must commit
        .analogue(9, 9, 9) // maximum certainty for behavioral decisions
        .context("gait")
        .operation("an") // analyze/decide
        .modifier("det") // deterministic output
        .input(format!(
            "planted={} desired={} margin={:.2}",
            planted_feet_input, desired_feet_input, stability_margin
        ))
        .constraint("stability")
        .constraint("support_check")
        .flag('!') // re-evaluate next frame — gait is stateful
        .build()
        .expect("gait_decision beam")
}

/// Transition between gaits (walk ↔ run ↔ hunt ↔ rest).
pub fn pass_gait_transition(current_gait: &str, environment_input: &str) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_RELEASE)
        .kappa(K_DECISION)
        .analogue(7, 5, 8)
        .context("gait_transition")
        .operation("an")
        .input(format!(
            "current={} env={}",
            current_gait, environment_input
        ))
        .constraint("energy_threshold")
        .flag('~') // transition flag — state changes persist
        .build()
        .expect("gait_transition beam")
}

// ============================================================================
// PASS 5: STEP TRAJECTORY — Animate foot between planted and target
// ============================================================================

/// Sinusoidal clearance curve for each stepping foot.
pub fn pass_step_trajectory(
    start_x: f32,
    start_y: f32,
    end_x: f32,
    end_y: f32,
    progress: f32,
    step_height: f32,
) -> MoifeuBeam {
    let height = format_float(step_height);
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_ANTI)
        .kappa(K_CTRL)
        .analogue(7, 8, 6) // high speed — trajectory is pure math, fast to compute
        .context("step_curve")
        .operation("sm") // simulate the curve
        .modifier("fml")
        .input(format!(
            "start={},{},{} end={},{},{} progress={:.3} height={}",
            start_x, start_y, 0.0, end_x, end_y, 0.0, progress, height
        ))
        .constraint("sinusoidal_clearance")
        .build()
        .expect("step_trajectory beam")
}

// ============================================================================
// PASS 6: IK SOLVER — FABRIK for each stepping leg chain
// ============================================================================

/// Solve leg chain via forward-backward iterative positioning.
/// Works on arbitrary chains — no spider knowledge required.
pub fn pass_ik_solve(hip_x: f32, hip_y: f32, foot_input: &str, segments_input: &str) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_SUSTAIN)
        .kappa(K_CTRL)
        .analogue(8, 9, 7) // high speed (iterations), moderate certainty
        .context("ik_solve")
        .operation("cmp") // compare positions
        .modifier("fix") // fix chain to target
        .input(format!(
            "root={},{},{} targets={} segments={}",
            hip_x, hip_y, 0.0, foot_input, segments_input
        ))
        .constraint("fabrik")
        .build()
        .expect("ik_solve beam")
}

/// Second IK pass — re-solve after body deformation shifts hip positions.
pub fn pass_ik_resolved(
    hip_x: f32,
    hip_y: f32,
    foot_input: &str,
    segments_input: &str,
) -> MoifeuBeam {
    let mut beam = pass_ik_solve(hip_x, hip_y, foot_input, segments_input);
    // Modify context to indicate this is a recovery pass
    beam.context = Some("ik_resolved".to_string());
    beam.modifiers = vec!["fix".to_string()];
    beam.certainty = 8; // higher certainty — must restore contact
    beam
}

// ============================================================================
// PASS 7: TENSION — Planted feet push back into body
// ============================================================================

/// Calculate tension from planted legs and apply to body anchors.
pub fn pass_tension_calculation(
    planted_feet_input: &str,
    leg_chains_input: &str,
    hip_positions_input: &str,
) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, SIM_MODE, PHASE_SUSTAIN)
        .kappa(K_BATCH)
        .analogue(8, 9, 8) // high weight — tension is the ring closure
        .context("tension")
        .operation("cmp")
        .modifier("fix")
        .input(format!(
            "anchors={} chains={} hips={}",
            planted_feet_input, leg_chains_input, hip_positions_input
        ))
        .constraint("planted_foot_anchor")
        .flag('~') // sustained tension — persists across substeps
        .build()
        .expect("tension_calculation beam")
}

// ============================================================================
// PASS 8: PERSONALITY FIELD — Parameter modulation (NOT geometry)
// ============================================================================

/// Personality modifies simulation parameters only.
/// Never sets joint positions. Never moves feet.
pub fn pass_personality_field(
    density: f32,
    stance: i8,
    angle: f32,
    hue: f32,
    saturation: f32,
    whisper_len: usize,
) -> MoifeuBeam {
    let modifier = if whisper_len > 40 { "fml" } else { "inf" };
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_ANTI)
        .kappa(K_PERSONALITY) // frontier — emotional weight
        .analogue(7, 6, 5)
        .context("personality")
        .operation("sm") // simulate parameter evolution
        .modifier(modifier)
        .input(format!(
            "density={:.2} stance={} angle={:.1} hue={:.1} sat={:.1}",
            density, stance, angle, hue, saturation
        ))
        .constraint("parameter_only") // INVARIANT: never geometry
        .flag('*') // emotional state flag
        .build()
        .expect("personality_field beam")
}

// ============================================================================
// PASS 9: COLOR / VISUAL STATE — Render parameters from simulation
// ============================================================================

/// Color field: pragonastatic ↔ sporagonastatic gradient trajectory.
pub fn pass_color_state(
    anchor_a_r: f32,
    anchor_a_g: f32,
    anchor_a_b: f32,
    anchor_b_r: f32,
    anchor_b_g: f32,
    anchor_b_b: f32,
    trajectory_phase: f32,
) -> MoifeuBeam {
    let aa = format_float(anchor_a_r);
    let ag = format_float(anchor_a_g);
    let ab = format_float(anchor_a_b);
    let ba = format_float(anchor_b_r);
    let bb = format_float(anchor_b_g);
    let bc = format_float(anchor_b_b);

    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, OUT_MODE, PHASE_SUSTAIN)
        .kappa(K_RENDER)
        .analogue(3, 3, 4) // low depth — visual is thin layer over simulation
        .context("color_field")
        .operation("gen") // generate visual state
        .modifier("tbl") // table output (structured color data)
        .input(format!(
            "anchor_a={},{},{} anchor_b={},{},{} phase={:.3}",
            aa, ag, ab, ba, bb, bc, trajectory_phase
        ))
        .constraint("color_field")
        .flag('~') // persistent visual state
        .build()
        .expect("color_state beam")
}

// ============================================================================
// PASS 10: WEB GRAPH — Orb-weaver web generation (optional)
// ============================================================================

/// Procedural orb-web via constraint network.
/// Same primitives as body — just different topology.
pub fn pass_web_generation(anchor_count: u8, spiral_turns: f32) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, CTRL_MODE, PHASE_REST)
        .kappa(K_CTRL)
        .analogue(5, 7, 6)
        .context("web_graph")
        .operation("cmp")
        .modifier("fix")
        .input(format!(
            "anchors={} spiral={:.1}",
            anchor_count, spiral_turns
        ))
        .constraint("strand_tension")
        .build()
        .expect("web_generation beam")
}

// ============================================================================
// PASS 11: RENDER OUTPUT — Simulation state → vertex data
// ============================================================================

/// Package simulation results for rendering. No simulation logic here.
pub fn pass_render_output(
    body_particles_input: &str,
    leg_vertices_input: &str,
    web_vertices_input: &str,
    color_state_input: &str,
) -> MoifeuBeam {
    MoifeuBeamBuilder::new()
        .header(GAIT_ALL, OUT_MODE, PHASE_SUSTAIN)
        .kappa(K_RENDER)
        .analogue(3, 5, 5) // low depth — just packaging, no computation
        .context("render_output")
        .operation("fmt") // format for rendering pipeline
        .modifier("tbl") // structured output
        .input(body_particles_input)
        .build()
        .expect("render_output beam")
}

// ============================================================================
// COMPLETE CREATURE LOOP — Wire all phases together
// ============================================================================

/// Generate the full update sequence for one frame.
/// Returns beams in order; each beam's output feeds the next phase's input.
pub struct CreatureBeams {
    pub body_integration: MoifeuBeam,
    pub constraint_solve_1: MoifeuBeam,
    pub terrain_query: MoifeuBeam,
    pub foot_targets: MoifeuBeam,
    pub foot_correction: MoifeuBeam,
    pub gait_decision: MoifeuBeam,
    pub step_trajectory: MoifeuBeam,
    pub ik_solve: MoifeuBeam,
    pub tension: MoifeuBeam,
    pub body_integration_2: MoifeuBeam, // post-tension re-integration
    pub constraint_solve_2: MoifeuBeam,
    pub ik_resolved: MoifeuBeam, // second IK pass
    pub personality: MoifeuBeam,
    pub color_state: MoifeuBeam,
    pub render_output: MoifeuBeam,
}

pub fn creature_update_frame(
    body_center_x: f32,
    body_center_y: f32,
    velocity_x: f32,
    velocity_y: f32,
    heading: f32,
    dt: f32,
    planted_feet_wire: &str,
    leg_lengths_wire: &str,
    cycle_phase: f32,
    step_height: f32,
    stability_margin: f32,
    density: f32,
    stance: i8,
    angle: f32,
    hue: f32,
    sat: f32,
    color_phase: f32,
) -> CreatureBeams {
    // PASS 1: Body integration
    let body = pass_body_integration(dt);
    let constraints_1 = pass_constraint_solve(8, 0.01);

    // PASS 2: Terrain
    let terrain = pass_terrain_query(body_center_x, body_center_y, 1.5);

    // PASS 3: Foot targeting
    let targets = pass_foot_targets(
        body_center_x,
        body_center_y,
        heading,
        velocity_x,
        velocity_y,
    );
    let corrected = pass_foot_terrain_correction(targets.to_wire());

    // PASS 4: Gait
    let gait = pass_gait_decision(
        planted_feet_wire,
        &corrected.output().unwrap_or_default(),
        stability_margin,
    );

    // PASS 5: Step trajectory
    let step = pass_step_trajectory(
        body_center_x,
        body_center_y,
        body_center_x + velocity_x * 0.3,
        body_center_y + velocity_y * 0.3,
        cycle_phase,
        step_height,
    );

    // PASS 6: IK solve (first pass)
    let hips = format!("{:.3},{:.3},0.0", body_center_x, body_center_y);
    let ik = pass_ik_solve(
        body_center_x,
        body_center_y,
        &step.to_wire(),
        leg_lengths_wire,
    );

    // PASS 7: Tension — ring closure
    let tension =
        pass_tension_calculation(planted_feet_wire, &ik.output().unwrap_or_default(), &hips);

    // PASS 8: Body re-integration (tension has deformed the body)
    let body_2 = pass_body_integration(dt);
    let constraints_2 = pass_constraint_solve(10, 0.005);

    // PASS 9: IK resolved (second pass — hips shifted)
    let ik_2 = pass_ik_resolved(
        body_center_x,
        body_center_y,
        &step.to_wire(),
        leg_lengths_wire,
    );

    // PASS 10: Personality
    let personality = pass_personality_field(density, stance, angle, hue, sat, 0);

    // PASS 11: Color / Visual state
    let color = pass_color_state(
        0.25,
        0.65,
        0.90, // pragonastatic (warm gold)
        0.15,
        0.30,
        0.75, // sporagonastatic (cool silver-blue)
        color_phase,
    );

    // PASS 12: Render output
    let render = pass_render_output(
        &body_2.to_wire(),
        &ik_2.to_wire(),
        "", // web optional
        &color.to_wire(),
    );

    CreatureBeams {
        body_integration: body,
        constraint_solve_1: constraints_1,
        terrain_query: terrain,
        foot_targets: targets,
        foot_correction: corrected,
        gait_decision: gait,
        step_trajectory: step,
        ik_solve: ik,
        tension: tension,
        body_integration_2: body_2,
        constraint_solve_2: constraints_2,
        ik_resolved: ik_2,
        personality: personality,
        color_state: color,
        render_output: render,
    }
}

// ============================================================================
// GAIT PATTERN DEFINITIONS
// ============================================================================

/// Extended gait states beyond basic alternating walk.
pub enum GaitPattern {
    /// Resting — minimal leg movement, folded posture
    Rest { abdomen_settled: bool },
    /// Alternating walk (default)
    Walk { speed: f32 },
    /// Faster gait — all legs can move, less support
    Run { speed: f32 },
    /// Hunt mode — high tension, wide stance
    Hunt { prey_distance: f32 },
    /// Web weaving behavior
    Weave { web_anchor: (f32, f32) },
    /// Startled / flee response
    Flee { direction: (f32, f32), urgency: u8 },
}

impl GaitPattern {
    /// Convert gait pattern to Moifeu context + constraint modifiers.
    pub fn to_context(&self) -> (&'static str, Vec<String>) {
        match self {
            GaitPattern::Rest { abdomen_settled } => {
                ("gait_rest", vec!["fold".to_string(), "settle".to_string()])
            }
            GaitPattern::Walk { speed } => {
                ("gait_walk", vec![format!("speed={}", format_float(*speed))])
            }
            GaitPattern::Run { speed } => (
                "gait_run",
                vec![
                    format!("speed={}", format_float(*speed)),
                    "minimal_support".to_string(),
                ],
            ),
            GaitPattern::Hunt { prey_distance } => (
                "gait_hunt",
                vec![
                    format!("prey_dist={:.2}", prey_distance),
                    "wide_stance".to_string(),
                    "high_tension".to_string(),
                ],
            ),
            GaitPattern::Weave { web_anchor } => (
                "gait_weave",
                vec![format!(
                    "anchor={},{:.2},{:.2}",
                    0, web_anchor.0, web_anchor.1
                )],
            ),
            GaitPattern::Flee { direction, urgency } => (
                "gait_flee",
                vec![
                    format!("dir={},{:.2},{:.2}", 0, direction.0, direction.1),
                    format!("urgency={}", urgency),
                ],
            ),
        }
    }

    /// Gait-specific step height multiplier (personality-modulated).
    pub fn step_height_factor(&self) -> f32 {
        match self {
            GaitPattern::Rest { .. } => 0.05,
            GaitPattern::Walk { speed } => 0.15 + *speed * 0.1,
            GaitPattern::Run { speed } => 0.3 + *speed * 0.2,
            GaitPattern::Hunt { .. } => 0.25,
            GaitPattern::Weave { .. } => 0.08,
            GaitPattern::Flee { urgency, .. } => 0.4 + (*urgency as f32) * 0.05,
        }
    }
}

// ============================================================================
// HELPERS
// ============================================================================

fn format_float(f: f32) -> String {
    format!("{:.3}", f)
}

// ============================================================================
// BEAM CHAIN — Compose beams into a single wire string for transmission
// ============================================================================

/// Chain multiple beams into a single MoifeuBeamBatch-compatible wire.
pub fn chain_beams(beams: &[MoifeuBeam]) -> String {
    beams
        .iter()
        .map(|b| emit_beam(b))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Parse a beam chain back into individual beams (for reception).
pub fn unchain_beams(wire: &str) -> Result<Vec<MoifeuBeam>, String> {
    wire.split('\n')
        .filter(|line| !line.trim().is_empty())
        .map(|line| parse_beam(line).map_err(|e| e.to_string()))
        .collect()
}
