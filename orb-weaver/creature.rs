// Procedural Orb-Weaver — High-Level Creature Controller
// Wires all Moifeu beam passes together as a single update loop.
// The spider owns simulation state; rendering never touches physics directly.
//
// This module is part of the orb-weaver sub-module tree under moifeu/.
// It uses sibling modules (beams, spectrum, gait_patterns) for simulation components.

use super::beams::{creature_update_frame, MoifeuBeamBuilder};
use super::gait_patterns::{Environment, GaitMode, WebState};
use super::spectrum::ColorField;

// Re-export types from sibling modules for convenient access
pub use super::beams::*;

// ============================================================================
// PARTICLE — Verlet-integrated soft body point
// ============================================================================

#[derive(Clone, Debug)]
pub struct Particle {
    pub current: [f32; 3],
    pub previous: [f32; 3],
    pub acceleration: [f32; 3],
    pub inverse_mass: f32, // 0.0 = immovable (terrain contact)
}

impl Particle {
    pub fn new(pos: [f32; 3], mass: f32) -> Self {
        let inv_mass = if mass > 0.0 { 1.0 / mass } else { 0.0 };
        Self {
            current: pos,
            previous: pos,
            acceleration: [0.0, 0.0, 0.0],
            inverse_mass: inv_mass,
        }
    }

    pub fn integrate(&mut self, dt: f32) {
        let vel_x = self.current[0] - self.previous[0];
        let vel_y = self.current[1] - self.previous[1];
        let vel_z = self.current[2] - self.previous[2];

        self.previous = self.current;
        self.current[0] += vel_x + self.acceleration[0] * dt * dt;
        self.current[1] += vel_y + self.acceleration[1] * dt * dt;
        self.current[2] += vel_z + self.acceleration[2] * dt * dt;

        // Reset acceleration for next frame
        self.acceleration = [0.0, 0.0, 0.0];
    }
}

// ============================================================================
// CONSTRAINT — Generic distance constraint between two particles
// ============================================================================

#[derive(Clone, Debug)]
pub struct DistanceConstraint {
    pub a: usize, // particle index
    pub b: usize, // particle index
    pub rest_length: f32,
    pub stiffness: f32, // 0-1, how rigid the connection
}

impl DistanceConstraint {
    pub fn solve(&self, particles: &mut [Particle]) -> f32 {
        let pa = &particles[self.a];
        let pb = &particles[self.b];

        let dx = pb.current[0] - pa.current[0];
        let dy = pb.current[1] - pa.current[1];
        let dz = pb.current[2] - pa.current[2];
        let dist = (dx * dx + dy * dy + dz * dz).sqrt();

        if dist < 1e-6 {
            return 0.0; // prevent division by zero
        }

        let diff = (self.rest_length - dist) / dist * self.stiffness;
        let ox = dx * diff;
        let oy = dy * diff;
        let oz = dz * diff;

        let wa = if pa.inverse_mass > 0.0 {
            pa.inverse_mass
        } else {
            0.0
        };
        let wb = if pb.inverse_mass > 0.0 {
            pb.inverse_mass
        } else {
            0.0
        };
        let w_inv = if (wa + wb) > 0.0 {
            1.0 / (wa + wb)
        } else {
            0.0
        };

        if wa > 0.0 {
            particles[self.a].current[0] -= ox * wb * w_inv;
            particles[self.a].current[1] -= oy * wb * w_inv;
            particles[self.a].current[2] -= oz * wb * w_inv;
        }
        if wb > 0.0 {
            particles[self.b].current[0] += ox * wa * w_inv;
            particles[self.b].current[1] += oy * wa * w_inv;
            particles[self.b].current[2] += oz * wa * w_inv;
        }

        diff // return error magnitude for convergence tracking
    }
}

// ============================================================================
// BODY ANCHOR — Leg attachment point on the soft body
// ============================================================================

#[derive(Clone, Debug)]
pub struct BodyAnchor {
    pub particle_indices: [usize; 2], // weighted average of 2 particles for smoothness
    pub weights: [f32; 2],            // normalized to sum to 1.0
    pub local_offset: [f32; 3],       // offset from centroid in body-local space
}

impl BodyAnchor {
    /// Get world position of this anchor from current particle positions.
    pub fn position(&self, particles: &[Particle]) -> [f32; 3] {
        let p0 = &particles[self.particle_indices[0]];
        let p1 = &particles[self.particle_indices[1]];
        [
            p0.current[0] * self.weights[0] + p1.current[0] * self.weights[1],
            p0.current[1] * self.weights[0] + p1.current[1] * self.weights[1],
            p0.current[2] * self.weights[0]
                + p1.current[2] * self.weights[1]
                + self.local_offset[2],
        ]
    }

    /// Average of two particle positions gives us the hip position for IK.
    pub fn to_ik_hip(&self, particles: &[Particle]) -> [f32; 3] {
        self.position(particles)
    }
}

// ============================================================================
// SOFT BODY — Thorax + abdomen as deformable particle network
// ============================================================================

#[derive(Clone, Debug)]
pub struct SoftBody {
    pub particles: Vec<Particle>,
    pub constraints: Vec<DistanceConstraint>,
    pub anchors: Vec<BodyAnchor>,
    pub center: [f32; 3],
}

impl SoftBody {
    /// Build a thorax ring + abdomen ring with cross-bracing.
    pub fn new_orb_weaver() -> Self {
        let mut particles = Vec::new();
        let mut constraints = Vec::new();
        let mut anchors = Vec::new();

        // Thorax ring — 6 particles in a hexagon
        let thorax_center = [0.0, 0.3, 0.0];
        let thorax_radius = 0.25;
        for i in 0..6 {
            let angle = (i as f32) / 6.0 * std::f32::consts::TAU;
            let pos = [
                thorax_center[0] + angle.cos() * thorax_radius,
                thorax_center[1] + angle.sin() * thorax_radius,
                thorax_center[2],
            ];
            particles.push(Particle::new(pos, 0.5));
        }

        // Connect thorax ring
        for i in 0..6 {
            let next = (i + 1) % 6;
            constraints.push(DistanceConstraint {
                a: i,
                b: next,
                rest_length: thorax_radius,
                stiffness: 0.8,
            });
        }

        // Internal thorax cross-braces for volume preservation
        for i in 0..3 {
            let opposite = (i + 3) % 6;
            constraints.push(DistanceConstraint {
                a: i,
                b: opposite,
                rest_length: thorax_radius * 1.73,
                stiffness: 0.4,
            });
        }

        // Abdomen ring — larger, 8 particles
        let abdomen_start = particles.len();
        let abdomen_center = [0.0, -0.5, 0.0];
        let abdomen_radius = 0.35;
        for i in 0..8 {
            let angle = (i as f32) / 8.0 * std::f32::consts::TAU;
            let pos = [
                abdomen_center[0] + angle.cos() * abdomen_radius,
                abdomen_center[1] + angle.sin() * abdomen_radius,
                abdomen_center[2],
            ];
            particles.push(Particle::new(pos, 0.8));
        }

        // Connect abdomen ring
        for i in 0..8 {
            let next = (i + 1) % 8;
            constraints.push(DistanceConstraint {
                a: abdomen_start + i,
                b: abdomen_start + next,
                rest_length: abdomen_radius,
                stiffness: 0.6,
            });
        }

        // Bridge constraints between thorax and abdomen
        for i in 0..4 {
            let thorax_idx = i * 2;
            let abdomen_idx = abdomen_start + i * 2;
            constraints.push(DistanceConstraint {
                a: thorax_idx,
                b: abdomen_idx,
                rest_length: 0.35,
                stiffness: 0.9,
            });
        }

        // Body anchors — attachment points for 8 legs (4 per side)
        let left_side_thorax = [0, 1, 2];
        let right_side_thorax = [3, 4, 5];
        for i in 0..4 {
            if i < 3 {
                let pi = left_side_thorax[i];
                anchors.push(BodyAnchor {
                    particle_indices: [pi, (pi + 1) % 6],
                    weights: [0.5, 0.5],
                    local_offset: [0.0, 0.0, -0.1],
                });
            } else {
                let pi = right_side_thorax[i - 3];
                anchors.push(BodyAnchor {
                    particle_indices: [pi, (pi + 1) % 6],
                    weights: [0.5, 0.5],
                    local_offset: [0.0, 0.0, -0.1],
                });
            }
        }

        // Compute initial center
        let mut cx = 0.0_f32;
        let mut cy = 0.0_f32;
        let mut cz = 0.0_f32;
        for p in &particles {
            cx += p.current[0];
            cy += p.current[1];
            cz += p.current[2];
        }
        let n = particles.len() as f32;

        Self {
            particles,
            constraints,
            anchors,
            center: [cx / n, cy / n, cz / n],
        }
    }

    /// Full simulation step — integrate + solve constraints iteratively.
    pub fn update(&mut self, dt: f32, constraint_iterations: u8, stiffness_mod: f32) {
        // Reset accelerations
        for p in &mut self.particles {
            p.acceleration = [0.0, -9.8 * p.inverse_mass, 0.0]; // gravity
        }

        // Integrate all particles
        for p in &mut self.particles {
            p.integrate(dt);
        }

        // Solve constraints iteratively
        for _ in 0..constraint_iterations {
            let total_error: f32 = self
                .constraints
                .iter()
                .map(|c| c.solve(&mut self.particles))
                .sum();

            // Early exit if converged
            if total_error < 0.01 {
                break;
            }
        }

        // Update center of mass
        let mut cx = 0.0_f32;
        let mut cy = 0.0_f32;
        let mut cz = 0.0_f32;
        for p in &self.particles {
            cx += p.current[0];
            cy += p.current[1];
            cz += p.current[2];
        }
        let n = self.particles.len() as f32;
        self.center = [cx / n, cy / n, cz / n];
    }

    /// Get rendered particle positions.
    pub fn vertices(&self) -> &Vec<Particle> {
        &self.particles
    }

    /// Get anchor positions for leg attachment.
    pub fn anchor_positions(&self) -> Vec<[f32; 3]> {
        self.anchors
            .iter()
            .map(|a| a.position(&self.particles))
            .collect()
    }
}

// ============================================================================
// LEG — Kinematic chain (4 segments: coxa → femur → tibia → foot)
// ============================================================================

#[derive(Clone, Debug)]
pub struct Leg {
    pub leg_index: u8, // 0-7
    pub side: i8,      // -1 = left, +1 = right
    pub segment_lengths: [f32; 3],
    pub joint_positions: Vec<[f32; 3]>,
    pub planted_position: Option<[f32; 3]>,
    pub desired_position: Option<[f32; 3]>,
    pub step_state: LegStepState,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LegStepState {
    Planted,
    Stepping { progress: f32 },
}

impl Leg {
    /// Create a new leg with given segment lengths.
    pub fn new(leg_index: u8, side: i8, segments: [f32; 3]) -> Self {
        Self {
            leg_index,
            side,
            segment_lengths: segments,
            joint_positions: Vec::new(),
            planted_position: None,
            desired_position: None,
            step_state: LegStepState::Planted,
        }
    }

    /// Solve this leg's chain from hip to foot using FABRIK.
    pub fn solve_ik(&mut self, hip: [f32; 3], target: [f32; 3]) {
        let mut chain = vec![hip];
        let total_len = self.segment_lengths.iter().sum::<f32>();
        let dir = [
            (target[0] - hip[0]) / total_len.max(0.001),
            (target[1] - hip[1]) / total_len.max(0.001),
            (target[2] - hip[2]) / total_len.max(0.001),
        ];

        // Forward pass: stretch to target
        for &len in &self.segment_lengths {
            let last = chain.last().unwrap();
            chain.push([
                last[0] + dir[0] * len,
                last[1] + dir[1] * len,
                last[2] + dir[2] * len,
            ]);
        }

        // Backward pass: anchor at hip
        for i in (1..chain.len()).rev() {
            let dx = chain[i - 1][0] - chain[i][0];
            let dy = chain[i - 1][1] - chain[i][1];
            let dz = chain[i - 1][2] - chain[i][2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < 0.001 {
                continue;
            }
            let scale = self.segment_lengths[i - 1] / dist;
            chain[i] = [
                chain[i - 1][0] + dx * scale,
                chain[i - 1][1] + dy * scale,
                chain[i - 1][2] + dz * scale,
            ];
        }

        // Forward pass again: re-stretch to target for accuracy
        for i in 1..chain.len() {
            let dx = chain[i - 1][0] - hip[0];
            let dy = chain[i - 1][1] - hip[1];
            let dz = chain[i - 1][2] - hip[2];
            let dist = (dx * dx + dy * dy + dz * dz).sqrt();
            if dist < 0.001 {
                continue;
            }
            let scale = self.segment_lengths[i - 1] / dist;
            let base = if i == 1 { hip } else { chain[i - 1] };
            chain[i] = [
                base[0] + dx * scale,
                base[1] + dy * scale,
                base[2] + dz * scale,
            ];
        }

        self.joint_positions = chain;
    }

    /// Get foot position from solved chain.
    pub fn foot_position(&self) -> Option<[f32; 3]> {
        self.joint_positions.last().copied()
    }

    /// Begin stepping to a new target.
    pub fn begin_step(&mut self, target: [f32; 3]) {
        self.desired_position = Some(target);
        self.step_state = LegStepState::Stepping { progress: 0.0 };
    }

    /// Advance step by delta time.
    pub fn update_step(&mut self, dt: f32) {
        if let LegStepState::Stepping { progress } = &mut self.step_state {
            *progress += dt;
            if *progress >= 1.0 {
                *progress = 1.0;
                self.planted_position = self.desired_position;
                self.step_state = LegStepState::Planted;
            }
        }
    }

    /// Step progress [0, 1].
    pub fn step_progress(&self) -> f32 {
        match &self.step_state {
            LegStepState::Planted => 1.0,
            LegStepState::Stepping { progress } => *progress,
        }
    }
}

// ============================================================================
// SPIDER — High-level controller wiring all systems together
// ============================================================================

#[derive(Clone, Debug)]
pub struct Spider {
    pub body: SoftBody,
    pub legs: Vec<Leg>,
    pub gait_mode: GaitMode,
    pub color_field: ColorField,
    pub movement_direction: (f32, f32),
    pub cycle_phase: f32,
    pub personality: PersonalityState,
}

#[derive(Clone, Debug)]
pub struct PersonalityState {
    pub density: f32,    // 0-9 — emotional weight
    pub stance: i8,      // -1/0/+1 — ebb/hold/surge
    pub angle: f32,      // 0-360 — orientation vector
    pub hue: f32,        // 0-100 — color temperature
    pub saturation: f32, // 0-1 — intensity
}

impl Default for PersonalityState {
    fn default() -> Self {
        Self {
            density: 5.0,
            stance: 0,
            angle: 0.0,
            hue: 50.0,
            saturation: 78.0,
        }
    }
}

impl Spider {
    pub fn new() -> Self {
        let mut legs = Vec::new();
        for i in 0..8 {
            let side = if i < 4 { -1 } else { 1 };
            // Leg segments: coxa=0.08, femur=0.25, tibia=0.35
            let segments = [0.08, 0.25, 0.35];
            legs.push(Leg::new(i, side, segments));
        }

        Self {
            body: SoftBody::new_orb_weaver(),
            legs,
            gait_mode: GaitMode::Walk {
                speed: 0.5,
                cycle_phase: 0.0,
            },
            color_field: ColorField::new(),
            movement_direction: (0.0, 0.0),
            cycle_phase: 0.0,
            personality: PersonalityState::default(),
        }
    }

    /// Main update loop — the closed mechanical ring in action.
    pub fn update(&mut self, dt: f32) {
        // Get gait parameters
        let step_base = self.gait_mode.step_height_base();
        let personality_mods = self.gait_mode.personality_coupling();
        let stiffness = self.gait_mode.body_stiffness() as f32;

        // PERSONALITY → color field phase (parameter-only modification)
        self.color_field.update(
            self.personality.density,
            1.0 - self.personality.density / 9.0, // curiosity inversely proportional to density
            0,
        );

        // MOVEMENT DIRECTION from gait mode
        let (vx, vy) = self.gait_mode.desired_velocity();
        self.movement_direction = (vx, vy);

        // === PASS 1: Body Integration ===
        self.body.update(dt, 8, 1.0);

        // === PASS 2-3: Terrain Query + Foot Targeting ===
        let anchors = self.body.anchor_positions();
        let step_spread =
            self.gait_mode.leg_spread() * (1.0 + personality_mods.leg_spread_mod * 0.5);

        // === PASS 4: Gait Decision (already decided by mode) ===
        // The gait_mode determines stepping_legs via stepping_legs()

        // === PASS 5-6: Step Trajectory + IK for each leg ===
        for i in 0..8 {
            let anchor = anchors[i];
            let hip = [anchor[0], anchor[1], anchor[2] + 0.05];

            if !self.gait_mode.stepping_legs()[i] {
                // Leg is planted — keep current position
                if self.legs[i].step_progress() >= 1.0 {
                    self.legs[i].solve_ik(hip, hip);
                }
                continue;
            }

            // This leg is stepping
            let target_x = anchor[0] + vx * 0.3 * step_spread * (self.legs[i].side as f32);
            let target_y = anchor[1] + vy * 0.3 * step_spread;
            let target_z = self.personality.density / 9.0 * 0.15; // density affects body height

            match &self.legs[i].step_state {
                LegStepState::Planted => {
                    let mut target = [target_x, target_y, anchor[2]];
                    // Add sinusoidal clearance
                    if self.cycle_phase < 1.0 {
                        let clearance = (self.cycle_phase * std::f32::consts::PI).sin() * step_base;
                        target[2] += clearance;
                    }
                    self.legs[i].begin_step(target);
                }
                LegStepState::Stepping { .. } => {}
            }

            self.legs[i].solve_ik(hip, [target_x, target_y, target_z]);
        }

        // === PASS 7: Tension Calculation ===
        // Planted legs apply tension to body anchors
        for i in 0..8 {
            if !self.gait_mode.stepping_legs()[i] {
                if let Some(foot) = self.legs[i].foot_position() {
                    let anchor = anchors[i];
                    let dx = foot[0] - anchor[0];
                    let dy = foot[1] - anchor[1];
                    let dz = foot[2] - anchor[2];
                    let tension = (dx * dx + dy * dy + dz * dz).sqrt() * 0.7;

                    if tension > 0.01 {
                        // Apply to body particles via anchor's particle_indices
                        for &pi in &self.body.anchors[i].particle_indices {
                            if pi < self.body.particles.len() {
                                let len = (dx * dx + dy * dy).sqrt().max(0.001);
                                self.body.particles[pi].acceleration[0] -=
                                    dx / len * tension * self.body.particles[pi].inverse_mass;
                                self.body.particles[pi].acceleration[1] -=
                                    dy / len * tension * self.body.particles[pi].inverse_mass;
                            }
                        }
                    }
                }
            }
        }

        // === PASS 8: Body Re-Integration (post-tension) ===
        let constraint_iters = if self.personality.density > 7.0 {
            12u8
        } else {
            8
        };
        self.body.update(dt, constraint_iters, 1.0);

        // === PASS 9: IK Resolved (second pass — hips moved after tension) ===
        let anchors_2 = self.body.anchor_positions();
        for i in 0..8 {
            let hip = [anchors_2[i][0], anchors_2[i][1], anchors_2[i][2] + 0.05];
            if let Some(desired) = self.legs[i].desired_position {
                self.legs[i].solve_ik(hip, desired);
            }
        }

        // Advance cycle phase
        let step_freq = personality_mods.step_frequency * (self.personality.density / 5.0);
        self.cycle_phase += dt * step_freq;
        if self.cycle_phase >= 1.0 {
            self.cycle_phase -= 1.0;
        }

        // === Gait Transition Check ===
        let env = Environment {
            perceived_threat: 0.0, // TODO: hook up to sensor input
            threat_direction: (0.0, 0.0),
            threat_distance: f32::INFINITY,
            has_prey: false,
            prey_distance: None,
            prey_direction: None,
            web_state: WebState::None,
        };
        self.gait_mode = self.gait_mode.transition(&env);
    }

    /// Exposed render state — pure output, no simulation logic.
    pub fn render_state(&self) -> RenderState {
        RenderState {
            body_vertices: self.body.vertices().iter().map(|p| p.current).collect(),
            leg_vertices: self
                .legs
                .iter()
                .flat_map(|l| l.joint_positions.clone())
                .collect(),
            color: self.color_field.sample(),
            visual_mode: if self.gait_mode.body_stiffness() > 6 {
                VisualMode::Active
            } else {
                VisualMode::Settled
            },
        }
    }

    /// Emit the full creature state as Moifeu beams.
    pub fn to_beams(&self) -> [MoifeuBeam; 5] {
        use crate::moifeu::{emit_beam, MoifeuBeamBuilder};

        let body_wire = emit_beam(
            &MoifeuBeamBuilder::new()
                .header(0, 0, 1)
                .kappa('b')
                .analogue(6, 8, 8)
                .context("body_vertices")
                .operation("fmt")
                .modifier("tbl")
                .input(format!(
                    "center={:.2},{:.2},{:.2} particles={}",
                    self.body.center[0],
                    self.body.center[1],
                    self.body.center[2],
                    self.body.particles.len()
                ))
                .build()
                .expect("body beam"),
        );

        let gait_wire = self.gait_mode.to_gait_beam();

        let personality_wire = emit_beam(
            &MoifeuBeamBuilder::new()
                .header(2, 1, 2)
                .kappa('r')
                .analogue(7, 6, 5)
                .context("personality")
                .operation("sm")
                .modifier("inf")
                .input(format!(
                    "density={:.2} stance={} angle={:.1} hue={:.1}",
                    self.personality.density,
                    self.personality.stance,
                    self.personality.angle,
                    self.personality.hue
                ))
                .constraint("parameter_only")
                .flag('*')
                .build()
                .expect("personality beam"),
        );

        let color_wire = self.color_field.to_wire();

        let leg_wire = emit_beam(
            &MoifeuBeamBuilder::new()
                .header(2, 1, 2)
                .kappa('w')
                .analogue(3, 5, 4)
                .context("leg_render")
                .operation("fmt")
                .modifier("tbl")
                .input(format!(
                    "chains={} mode={}",
                    self.legs.len(),
                    match &self.gait_mode {
                        GaitMode::Walk { .. } => "walk",
                        _ => "other",
                    }
                ))
                .build()
                .expect("leg beam"),
        );

        [
            body_wire.parse().expect("body wire"),
            gait_wire.parse().expect("gait wire"),
            personality_wire,
            color_wire.parse().expect("color wire"),
            leg_wire,
        ]
    }
}

// ============================================================================
// RENDER STATE — Output-only, no simulation logic
// ============================================================================

#[derive(Clone, Debug)]
pub struct RenderState {
    pub body_vertices: Vec<[f32; 3]>,
    pub leg_vertices: Vec<[f32; 3]>,
    pub color: [f32; 4], // r, g, b, emissive
    pub visual_mode: VisualMode,
}

#[derive(Clone, Debug)]
pub enum VisualMode {
    Active,  // high stiffness, visible tension
    Settled, // low stiffness, calm appearance
}

// ============================================================================
// WASM BOUNDARY — Minimal public interface for JavaScript
// ============================================================================

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
impl Spider {
    /// Update the spider by one frame. Returns encoded render state as wire string.
    pub fn wasm_update(&mut self, dt: f32) -> String {
        self.update(dt);
        let state = self.render_state();
        // In production, this would serialize to a binary format for WASM interop
        format!(
            "{},{},{:.4},{:.4},{:.4}",
            state.body_vertices.len(),
            state.leg_vertices.len(),
            state.color[0],
            state.color[1],
            state.color[2]
        )
    }
}
