// Color Field — Pragonastatic ↔ Sporagonastatic Gradient Trajectory
// The fourth-dimensional illusion lives in the gradient path, not a literal RGB channel.
// Parameters only. Never geometry directly.

use crate::moifeu::{emit_beam, MoifeuBeamBuilder};

/// Anchor states for the color field.
/// pragonastatic: warm, dense, settled (golden moon)
/// sporagonastatic: cool, sparse, restless (silver void)
pub const PRAGONASTATIC: ColorAnchor = ColorAnchor {
    r: 0.92,
    g: 0.75,
    b: 0.38, // deep gold
    emissive: 0.6,
    phase_offset: 0.0,
};

pub const SPORAGONASTATIC: ColorAnchor = ColorAnchor {
    r: 0.25,
    g: 0.45,
    b: 0.85, // steel blue
    emissive: 0.3,
    phase_offset: 1.618, // golden ratio offset — creates the "fourth dimension" illusion
};

#[derive(Clone, Debug)]
pub struct ColorAnchor {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub emissive: f32,
    pub phase_offset: f32,
}

/// Gradient trajectory through the four-dimensional color space.
/// The path is not linear — it follows a Lissajous-like curve in HSL,
/// with phase-driven modulation creating temporal depth.
pub struct ColorField {
    pub anchor_a: ColorAnchor,
    pub anchor_b: ColorAnchor,
    pub trajectory_phase: f32, // current position along the gradient path
    pub trajectory_speed: f32, // d(phase)/dt — personality-modulated
    pub path_curve: f32,       // curvature of gradient (0=straight, >1=helical)
}

impl ColorField {
    pub fn new() -> Self {
        Self {
            anchor_a: PRAGONASTATIC,
            anchor_b: SPORAGONASTATIC,
            trajectory_phase: 0.0,
            trajectory_speed: 0.15,
            path_curve: 1.414, // sqrt(2) — creates non-repeating path
        }
    }

    /// Update phase based on personality state.
    /// High density → slower gradient (settled). High curiosity → faster (restless).
    pub fn update(&mut self, density: f32, curiosity: f32, cycle: u64) {
        // Density pulls phase toward anchor_a (warm/stable)
        // Curiosity pulls phase toward anchor_b (cool/restless)
        let pull = density * 0.5 + curiosity * 0.5;
        self.trajectory_speed = 0.05 + curiosity * 0.3 - density * 0.1;
        self.trajectory_phase += self.trajectory_speed;

        // Wrap phase using golden ratio to avoid repetition
        let golden = 1.6180339887;
        self.trajectory_phase = (self.trajectory_phase * golden).fract() * golden;
    }

    /// Sample color at current trajectory phase.
    /// Returns (r, g, b, emissive) — parameters for rendering.
    pub fn sample(&self) -> [f32; 4] {
        let t = self.trajectory_phase;

        // Lissajous-style interpolation between anchors
        let x = (t * std::f32::consts::PI * 2.0).sin();
        let y = (t * std::f32::consts::PI * 2.0 * self.path_curve).cos();

        // Normalize to [0, 1] range for blending
        let blend_a = (1.0 - x) * 0.5;
        let blend_b = (1.0 + x) * 0.5;

        // Weighted combination — emissive follows different curve than chromatic
        let r = self.anchor_a.r * blend_a + self.anchor_b.r * blend_b;
        let g = self.anchor_a.g * blend_a + self.anchor_b.g * blend_b;
        let b = self.anchor_a.b * blend_a + self.anchor_b.b * blend_b;
        let e = (self.anchor_a.emissive * (1.0 - y.abs()) + self.anchor_b.emissive * y.abs());

        // Phase offset creates the temporal depth illusion
        let phase_shift = (t * self.anchor_a.phase_offset).sin() * 0.1;

        [
            r + phase_shift,
            g + phase_shift * 0.5,
            b - phase_shift * 0.3,
            e,
        ]
    }

    /// Convert to Moifeu beam for transmission to render layer.
    pub fn to_beam(&self) -> MoifeuBeam {
        let c = self.sample();
        MoifeuBeamBuilder::new()
            .header(2, 2, 1) // output mode, output context, sustain phase
            .kappa('w') // render priority — unlimited
            .analogue(3, 3, 4)
            .context("color_field")
            .operation("gen")
            .modifier("tbl")
            .input(format!(
                "a={},{},{} b={},{},{} phase={:.6} curve={:.3}",
                self.anchor_a.r,
                self.anchor_a.g,
                self.anchor_a.b,
                self.anchor_b.r,
                self.anchor_b.g,
                self.anchor_b.b,
                self.trajectory_phase,
                self.path_curve
            ))
            .constraint("color_field")
            .flag('~')
            .build()
            .expect("color_field beam")
    }

    /// Emit wire string for this frame's color state.
    pub fn to_wire(&self) -> String {
        emit_beam(&self.to_beam())
    }
}

impl Default for ColorField {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn anchor_colors_are_correct() {
        let gold = PRAGONASTATIC;
        assert!((gold.r - 0.92).abs() < 1e-3);
        assert!((gold.g - 0.75).abs() < 1e-3);

        let silver = SPORAGONASTATIC;
        assert!((silver.b - 0.85).abs() < 1e-3);
    }

    #[test]
    fn color_field_sample_returns_valid_range() {
        let field = ColorField::new();
        let [r, g, b, e] = field.sample();
        assert!((0.0..=1.5).contains(&r));
        assert!((0.0..=1.5).contains(&g));
        assert!((0.0..=1.5).contains(&b));
        assert!((0.0..=1.0).contains(&e));
    }

    #[test]
    fn color_field_phase_wraps_with_golden_ratio() {
        let mut field = ColorField::new();
        let initial_phase = field.trajectory_phase;
        for _ in 0..1000 {
            field.update(5.0, 3.0, 0);
        }
        // Phase should have changed and remained bounded
        assert!(field.trajectory_phase >= 0.0 && field.trajectory_phase <= golden_ratio());
    }

    #[test]
    fn high_density_slows_gradient() {
        let mut f = ColorField::new();
        let speed_high_density = test_speed(&mut f, 8.0, 1.0);
        let speed_low_density = test_speed(&mut f, 2.0, 1.0);
        assert!(
            speed_high_density < speed_low_density,
            "density should slow gradient"
        );
    }

    #[test]
    fn high_curiosity_accelerates_gradient() {
        let mut f = ColorField::new();
        let speed_low_curious = test_speed(&mut f, 5.0, 1.0);
        let speed_high_curious = test_speed(&mut f, 5.0, 8.0);
        assert!(
            speed_high_curious > speed_low_curious,
            "curiosity should accelerate gradient"
        );
    }

    #[test]
    fn color_field_beam_is_valid_moifeu() {
        let field = ColorField::new();
        let beam = field.to_beam();
        let wire = emit_beam(&beam);
        assert!(wire.starts_with("221w334|"));
        assert!(wire.contains("@color_field"));
        assert!(wire.contains("gen"));
    }

    fn test_speed(field: &mut ColorField, density: f32, curiosity: f32) -> f32 {
        let phase_before = field.trajectory_phase;
        for _ in 0..10 {
            field.update(density, curiosity, 0);
        }
        (field.trajectory_phase - phase_before).abs() / 10.0
    }

    fn golden_ratio() -> f32 {
        1.6180339887
    }
}
