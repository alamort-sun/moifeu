// Extended Gait Patterns for Procedural Orb-Weaver
// Each pattern defines leg grouping, timing, step parameters, and personality coupling.

/// Movement modes the spider can inhabit.
#[derive(Clone, Debug, PartialEq)]
pub enum GaitMode {
    /// Resting — legs fold inward, abdomen settles, minimal movement
    Rest {
        /// Whether abdomen has settled to ground plane
        abdomen_settled: bool,
        /// Ambient tension level (0-1) affecting subtle micro-movements
        ambient_tension: f32,
    },
    /// Standard alternating walk — classic spider gait
    Walk {
        /// Speed multiplier (0.5 = half-speed amble, 1.0 = normal)
        speed: f32,
        /// Current phase in the alternating cycle [0, 1)
        cycle_phase: f32,
    },
    /// Faster gait — reduced support legs, more body bounce
    Run {
        speed: f32,
        cycle_phase: f32,
        /// Whether body is elevated (true during full sprint)
        body_elevated: bool,
    },
    /// Hunt mode — wide stance, high tension, deliberate movement
    Hunt {
        prey_distance: f32,
        /// Prey direction in world space
        prey_dir: (f32, f32),
        cycle_phase: f32,
    },
    /// Web building behavior
    Weave {
        web_center: (f32, f32),
        current_radial: u8,
        spiral_progress: f32,
    },
    /// Startled / flee response
    Flee {
        direction: (f32, f32),
        urgency: u8, // 1-9, scales speed and tension
        remaining_steps: u8,
    },
}

impl GaitMode {
    // ==================== BEHAVIORAL OUTPUTS ====================

    /// Desired movement direction from body center.
    pub fn desired_velocity(&self) -> (f32, f32) {
        match self {
            GaitMode::Rest { .. } => (0.0, 0.0),
            GaitMode::Walk { speed, cycle_phase } => {
                let phase_offset = cycle_phase * std::f32::consts::TAU;
                (
                    speed * 0.5 * phase_offset.sin(),
                    speed * 0.3 * phase_offset.cos(),
                )
            }
            GaitMode::Run {
                speed, cycle_phase, ..
            } => {
                let phase_offset = cycle_phase * std::f32::consts::TAU;
                (
                    speed * 1.5 * phase_offset.sin(),
                    speed * 0.8 * phase_offset.cos(),
                )
            }
            GaitMode::Hunt {
                prey_dir,
                cycle_phase,
                ..
            } => {
                let (px, py) = *prey_dir;
                let mag = (px * px + py * py).sqrt().max(0.01);
                let approach = 1.0 - (py / mag).clamp(0.0, 1.0);
                let side_swerve = (cycle_phase * std::f32::consts::TAU).sin() * 0.3;
                (
                    (px / mag) * approach + side_swerve * (-py / mag),
                    (py / mag) * approach + side_swerve * (px / mag),
                )
            }
            GaitMode::Weave {
                web_center,
                current_radial,
                spiral_progress,
            } => {
                let angle = (*current_radial as f32) / 8.0 * std::f32::consts::TAU;
                let radius = 0.5 + spiral_progress * 2.0;
                (
                    web_center.0 + angle.cos() * radius,
                    web_center.1 + angle.sin() * radius,
                )
            }
            GaitMode::Flee {
                direction, urgency, ..
            } => {
                let (dx, dy) = *direction;
                let mag = (dx * dx + dy * dy).sqrt().max(0.01);
                let speed_factor = (1 + *urgency as f32) * 2.0 / 10.0;
                ((dx / mag) * speed_factor, (dy / mag) * speed_factor)
            }
        }
    }

    /// Which legs may step during this mode.
    pub fn stepping_legs(&self) -> [bool; 8] {
        match self {
            GaitMode::Rest { .. } => [false, false, false, false, false, false, false, false],
            // Alternating: A-legs (L3, L1, R1, R3) vs B-legs (L4, L2, R4, R2)
            GaitMode::Walk { cycle_phase, .. } | GaitMode::Run { cycle_phase, .. } => {
                let group_a = *cycle_phase < 0.5;
                // L4=0, L3=1, L2=2, L1=3, R1=4, R2=5, R3=6, R4=7
                [
                    !group_a, group_a, !group_a, group_a, // left legs
                    group_a, !group_a, group_a, !group_a, // right legs
                ]
            }
            GaitMode::Hunt { .. } => {
                // Wider stance — more legs planted for stability
                [false, true, false, true, true, false, true, false]
            }
            GaitMode::Weave { .. } => {
                // Deliberate placement — one leg at a time
                [true, false, true, false, false, true, false, true]
            }
            GaitMode::Flee { urgency, .. } => {
                // All legs available, higher urgency = less support needed
                let all_legs = *urgency >= 7;
                if all_legs {
                    [true, true, true, true, true, true, true, true]
                } else {
                    [true, false, true, false, false, true, false, true]
                }
            }
        }
    }

    /// Step height multiplier (0-1) — personality-modulated later.
    pub fn step_height_base(&self) -> f32 {
        match self {
            GaitMode::Rest { .. } => 0.05,
            GaitMode::Walk { speed, .. } => 0.15 + speed * 0.1,
            GaitMode::Run { speed, .. } => 0.3 + speed * 0.25,
            GaitMode::Hunt { prey_distance, .. } => {
                0.2 + if prey_distance < 1.0 { 0.15 } else { 0.0 }
            }
            GaitMode::Weave { .. } => 0.08,
            GaitMode::Flee { urgency, .. } => 0.35 + *urgency as f32 * 0.03,
        }
    }

    /// Body stiffness multiplier for constraint solver (1-10).
    /// Higher tension modes require stiffer body to transmit leg force.
    pub fn body_stiffness(&self) -> u8 {
        match self {
            GaitMode::Rest { .. } => 3,
            GaitMode::Walk { .. } => 5,
            GaitMode::Run { .. } => 7,
            GaitMode::Hunt { .. } => 8,
            GaitMode::Weave { .. } => 4,
            GaitMode::Flee { urgency, .. } => (6 + *urgency / 3).min(9),
        }
    }

    /// Leg spread offset from body center (affects foot targeting radius).
    pub fn leg_spread(&self) -> f32 {
        match self {
            GaitMode::Rest { .. } => 0.4,
            GaitMode::Walk { .. } => 0.6,
            GaitMode::Run { .. } => 0.8,
            GaitMode::Hunt { .. } => 0.9,
            GaitMode::Weave { .. } => 0.5,
            GaitMode::Flee { .. } => 1.0,
        }
    }

    /// Personality parameter modifiers for this gait mode.
    pub fn personality_coupling(&self) -> PersonalityParams {
        match self {
            GaitMode::Rest {
                ambient_tension, ..
            } => PersonalityParams {
                step_frequency: 0.3,
                body_stiffness_mod: -0.4,
                step_height_mod: -0.8,
                leg_spread_mod: -0.5,
                idle_motion_amplitude: *ambient_tension * 0.1,
            },
            GaitMode::Walk { speed, .. } => PersonalityParams {
                step_frequency: 0.6 + *speed * 0.4,
                body_stiffness_mod: 0.0,
                step_height_mod: *speed * 0.2,
                leg_spread_mod: *speed * 0.1,
                idle_motion_amplitude: 0.05,
            },
            GaitMode::Run { speed, .. } => PersonalityParams {
                step_frequency: 0.8 + *speed * 0.2,
                body_stiffness_mod: 0.3,
                step_height_mod: *speed * 0.3,
                leg_spread_mod: 0.2,
                idle_motion_amplitude: 0.15,
            },
            GaitMode::Hunt { prey_distance, .. } => PersonalityParams {
                step_frequency: if prey_distance < 2.0 { 0.9 } else { 0.7 },
                body_stiffness_mod: 0.4,
                step_height_mod: 0.1,
                leg_spread_mod: 0.3,
                idle_motion_amplitude: 0.0, // no idle motion when hunting
            },
            GaitMode::Weave { .. } => PersonalityParams {
                step_frequency: 0.4,
                body_stiffness_mod: -0.2,
                step_height_mod: -0.6,
                leg_spread_mod: -0.3,
                idle_motion_amplitude: 0.1,
            },
            GaitMode::Flee { urgency, .. } => PersonalityParams {
                step_frequency: (0.7 + *urgency as f32 * 0.03).min(1.0),
                body_stiffness_mod: (*urgency as f32) * 0.05,
                step_height_mod: (*urgency as f32) * 0.04,
                leg_spread_mod: (*urgency as f32) * 0.06,
                idle_motion_amplitude: 0.3 + *urgency as f32 * 0.05,
            },
        }
    }

    /// Convert to Moifeu beam for gait decision pass.
    pub fn to_gait_beam(&self) -> String {
        use crate::moifeu::{emit_beam, MoifeuBeamBuilder};

        let (context, mods) = match self {
            GaitMode::Rest { .. } => (
                "gait_rest".to_string(),
                vec!["fold".into(), "settle".into()],
            ),
            GaitMode::Walk { speed, phase } => (
                "gait_walk".to_string(),
                vec![
                    format!("speed={}", format_float(*speed)),
                    format!("phase={:.3}", phase),
                ],
            ),
            GaitMode::Run {
                speed,
                phase,
                elevated,
            } => (
                "gait_run".to_string(),
                vec![
                    format!("speed={}", format_float(*speed)),
                    format!("phase={:.3}", phase),
                    if *elevated {
                        "elevated".into()
                    } else {
                        "".into()
                    },
                ],
            ),
            GaitMode::Hunt {
                prey_distance,
                prey_dir,
                phase,
            } => (
                "gait_hunt".to_string(),
                vec![
                    format!("prey_dist={:.2}", prey_distance),
                    format!("dir={},{:.2}", prey_dir.0, prey_dir.1),
                    "wide_stance".into(),
                    "high_tension".into(),
                ],
            ),
            GaitMode::Weave {
                web_center,
                radial,
                spiral,
            } => (
                "gait_weave".to_string(),
                vec![
                    format!("anchor={:.2},{:.2}", web_center.0, web_center.1),
                    format!("radial={}", radial),
                    format!("spiral={:.3}", spiral),
                ],
            ),
            GaitMode::Flee {
                direction,
                urgency,
                remaining,
            } => (
                "gait_flee".to_string(),
                vec![
                    format!("dir={},{:.2}", direction.0, direction.1),
                    format!("urgency={}", urgency),
                    format!("remaining_steps={}", remaining),
                ],
            ),
        };

        let mut beam = MoifeuBeamBuilder::new()
            .header(2, 1, 2) // broadcast all, control mode, anticipation phase
            .kappa('y') // urgent decision
            .analogue(9, 9, 9)
            .context(&context)
            .operation("an")
            .modifier("det")
            .input(mods.join(" "))
            .constraint("stability")
            .flag('!')
            .build()
            .expect("gait beam");

        emit_beam(&beam)
    }
}

/// Personality parameter modifications driven by gait mode.
/// These values are added to base personality state, not set absolutely.
#[derive(Clone, Debug)]
pub struct PersonalityParams {
    pub step_frequency: f32,        // modifier for gait controller timing
    pub body_stiffness_mod: f32,    // modifier for constraint solver
    pub step_height_mod: f32,       // modifier for trajectory curve
    pub leg_spread_mod: f32,        // modifier for foot targeting offset
    pub idle_motion_amplitude: f32, // micro-movement when stationary
}

impl PersonalityParams {
    /// Apply personality field modifiers to base gait parameters.
    pub fn apply(
        self,
        base_step_freq: f32,
        base_stiffness: u8,
        base_height: f32,
    ) -> GaitParameters {
        GaitParameters {
            step_frequency: (base_step_freq * self.step_frequency).clamp(0.1, 2.0),
            body_stiffness: (base_stiffness as f32 + self.body_stiffness_mod * 5.0).clamp(1.0, 9.0)
                as u8,
            step_height: (base_height + self.step_height_mod).clamp(0.02, 1.0),
        }
    }
}

/// Final gait parameters after personality modulation.
pub struct GaitParameters {
    pub step_frequency: f32,
    pub body_stiffness: u8,
    pub step_height: f32,
}

// ==================== GAIT TRANSITION LOGIC ====================

impl GaitMode {
    /// Determine next gait mode based on environment and internal state.
    pub fn transition(self, env: &Environment) -> GaitMode {
        match self {
            GaitMode::Rest { .. } => {
                if env.perceived_threat > 0.7 {
                    GaitMode::Flee {
                        direction: (env.threat_direction.0 * -1.0, env.threat_direction.1 * -1.0),
                        urgency: (env.perceived_threat * 9.0) as u8,
                        remaining_steps: (env.threat_distance / 2.0) as u8,
                    }
                } else if env.has_prey {
                    GaitMode::Hunt {
                        prey_distance: env.prey_distance.unwrap_or(5.0),
                        prey_dir: env.prey_direction.unwrap_or((1.0, 0.0)),
                        cycle_phase: 0.0,
                    }
                } else if env.web_state == WebState::NeedsRepair {
                    GaitMode::Weave {
                        web_center: (0.0, 0.0),
                        current_radial: 0,
                        spiral_progress: 0.0,
                    }
                } else {
                    self // remain resting
                }
            }
            GaitMode::Walk { speed, cycle_phase } => {
                if env.perceived_threat > 0.5 {
                    GaitMode::Flee {
                        direction: (env.threat_direction.0 * -1.0, env.threat_direction.1 * -1.0),
                        urgency: (env.perceived_threat * 7.0) as u8,
                        remaining_steps: 60,
                    }
                } else if speed < 0.3 && !env.has_prey {
                    GaitMode::Rest {
                        abdomen_settled: false,
                        ambient_tension: 0.1,
                    }
                } else {
                    self // continue walking
                }
            }
            GaitMode::Hunt {
                prey_distance,
                prey_dir,
                cycle_phase,
            } => {
                if !env.has_prey || env.prey_distance.unwrap_or(f32::INFINITY) > 10.0 {
                    GaitMode::Walk {
                        speed: 0.5,
                        cycle_phase,
                    }
                } else if prey_distance < 0.5 {
                    GaitMode::Flee {
                        direction: prey_dir, // prey is too close — flee!
                        urgency: 9,
                        remaining_steps: 30,
                    }
                } else {
                    self // continue hunting
                }
            }
            GaitMode::Flee {
                mut remaining_steps,
                urgency,
                ..
            } => {
                if remaining_steps == 0 || env.perceived_threat < 0.2 {
                    GaitMode::Walk {
                        speed: 0.5,
                        cycle_phase: 0.0,
                    }
                } else {
                    GaitMode::Flee {
                        remaining_steps: remaining_steps - 1,
                        urgency,
                        .. /* keep direction */
                    }
                }
            }
            GaitMode::Weave {
                web_center,
                current_radial,
                spiral_progress,
            } => {
                if env.web_state == WebState::Complete && env.perceived_threat < 0.3 {
                    GaitMode::Rest {
                        abdomen_settled: true,
                        ambient_tension: 0.05,
                    }
                } else if spiral_progress >= 1.0 {
                    GaitMode::Walk {
                        speed: 0.3,
                        cycle_phase: 0.0,
                    }
                } else {
                    self // continue weaving
                }
            }
            GaitMode::Run { .. } => {
                self // run continues until environment changes dramatically
            }
        }
    }
}

/// Environmental state influencing gait transitions.
pub struct Environment {
    pub perceived_threat: f32,        // 0-1
    pub threat_direction: (f32, f32), // normalized direction of threat
    pub threat_distance: f32,         // distance to nearest threat
    pub has_prey: bool,
    pub prey_distance: Option<f32>,
    pub prey_direction: Option<(f32, f32)>,
    pub web_state: WebState,
}

/// Web state for transition logic.
#[derive(Clone, Debug, PartialEq)]
pub enum WebState {
    None,
    Building { progress: f32 },
    Complete,
    Damaged { integrity: f32 }, // 0-1
    NeedsRepair,
}

// ==================== HELPERS ====================

fn format_float(f: f32) -> String {
    format!("{:.3}", f)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rest_has_no_stepping_legs() {
        let mode = GaitMode::Rest {
            abdomen_settled: true,
            ambient_tension: 0.1,
        };
        assert!(!mode.stepping_legs().iter().any(|&b| b));
    }

    #[test]
    fn walk_alternates_groups() {
        let phase_a = GaitMode::Walk {
            speed: 0.5,
            cycle_phase: 0.25,
        };
        let phase_b = GaitMode::Walk {
            speed: 0.5,
            cycle_phase: 0.75,
        };
        let legs_a = phase_a.stepping_legs();
        let legs_b = phase_b.stepping_legs();
        for i in 0..8 {
            assert_ne!(
                legs_a[i], legs_b[i],
                "A and B groups should be opposite at leg {}",
                i
            );
        }
    }

    #[test]
    fn hunt_has_wider_stance() {
        let hunt = GaitMode::Hunt {
            prey_distance: 3.0,
            prey_dir: (1.0, 0.0),
            cycle_phase: 0.5,
        };
        assert!(
            hunt.leg_spread()
                > GaitMode::Walk {
                    speed: 0.5,
                    cycle_phase: 0.5
                }
                .leg_spread()
        );
    }

    #[test]
    fn flee_urgency_scales_speed() {
        let low = GaitMode::Flee {
            direction: (1.0, 0.0),
            urgency: 2,
            remaining_steps: 30,
        };
        let high = GaitMode::Flee {
            direction: (1.0, 0.0),
            urgency: 9,
            remaining_steps: 30,
        };
        let (lx, ly) = low.desired_velocity();
        let (hx, hy) = high.desired_velocity();
        assert!((hx * hx + hy * hy) > (lx * lx + ly * ly));
    }

    #[test]
    fn personality_coupling_produces_valid_params() {
        let walk = GaitMode::Walk {
            speed: 0.8,
            cycle_phase: 0.3,
        };
        let pp = walk.personality_coupling();
        assert!(pp.step_frequency >= 0.0 && pp.step_frequency <= 1.5);
        assert!(-0.5..=1.0).contains(&pp.body_stiffness_mod);
    }

    #[test]
    fn gait_beam_starts_with_correct_header() {
        let mode = GaitMode::Walk {
            speed: 0.5,
            cycle_phase: 0.3,
        };
        let beam = mode.to_gait_beam();
        assert!(beam.starts_with("21y999|"));
    }
}
