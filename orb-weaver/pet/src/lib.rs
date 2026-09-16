pub mod behavior;
pub mod body;
pub mod ik;
pub mod locomotion;
pub mod math;
pub mod physics;
pub mod silk;
pub mod visual;
pub mod world;
use body::Body;
use locomotion::{Gait, Leg};
use math::V;
use world::Terrain;
pub struct Spider {
    pub body: Body,
    pub legs: Vec<Leg>,
    pub terrain: Terrain,
    pub behavior: behavior::Behavior,
    pub web: silk::Web,
    gait: Gait,
    heading: f32,
    goal: V,
    accumulator: f32,
    pub curiosity: f32,
    pub tension: f32,
    pub frame: Vec<f32>,
}
impl Default for Spider {
    fn default() -> Self {
        Self::new()
    }
}
impl Spider {
    pub fn new() -> Self {
        let body = Body::new();
        let terrain = Terrain::default();
        let legs = (0..8)
            .map(|i| {
                let (f, _) =
                    locomotion::foot_target(V::default(), 0.0, i, 1.0, V::default(), &terrain);
                Leg::new(body.hip(i), f, (i % 4 + i / 4) % 2)
            })
            .collect();
        let mut s = Self {
            body,
            legs,
            terrain,
            behavior: behavior::Behavior {
                bounds: V::new(4.0, 2.5, 0.0),
                ..Default::default()
            },
            web: Default::default(),
            gait: Default::default(),
            heading: 0.0,
            goal: V::default(),
            accumulator: 0.0,
            curiosity: 0.6,
            tension: 0.2,
            frame: vec![],
        };
        s.render();
        s
    }
    pub fn set_mode(&mut self, mode: u32) {
        self.behavior.mode = mode.min(3);
        if mode == 3 && self.web.nodes.is_empty() {
            let mut p = self.goal;
            p.z = 0.0;
            self.web.build(p)
        }
    }
    pub fn update(&mut self, dt: f32) {
        if !dt.is_finite() || dt <= 0.0 {
            return;
        }
        self.accumulator += dt.min(0.1);
        while self.accumulator >= 1.0 / 120.0 {
            self.tick(1.0 / 120.0);
            self.accumulator -= 1.0 / 120.0;
        }
        self.render();
    }
    fn tick(&mut self, dt: f32) {
        let motion = self.behavior.update(dt, self.goal);
        let p = behavior::personality(self.curiosity, self.tension, motion.rest);
        self.goal += motion.velocity * dt;
        self.goal.x = self
            .goal
            .x
            .clamp(-self.behavior.bounds.x, self.behavior.bounds.x);
        self.goal.y = self
            .goal
            .y
            .clamp(-self.behavior.bounds.y, self.behavior.bounds.y);
        if motion.velocity.len() > 0.05 {
            let target = motion.velocity.x.atan2(-motion.velocity.y);
            let delta = (target - self.heading + std::f32::consts::PI)
                .rem_euclid(std::f32::consts::TAU)
                - std::f32::consts::PI;
            self.heading += delta.clamp(-1.5 * dt, 1.5 * dt);
        }
        for i in 0..8 {
            if !self.legs[i].moving {
                let (target, normal) = locomotion::foot_target(
                    self.goal,
                    self.heading,
                    i,
                    p.spread,
                    motion.velocity,
                    &self.terrain,
                );
                self.legs[i].desired = target;
                self.legs[i].normal = normal;
            }
        }
        self.gait.request(&mut self.legs, 0.20);
        for i in 0..8 {
            let hip = self.body.hip(i);
            self.legs[i].update(dt, p.duration, p.lift);
            self.legs[i].solve(hip);
            if !self.legs[i].moving {
                let force = physics::tension(hip, self.legs[i].planted, 0.66, 110.0);
                let k = Body::hip_index(i);
                self.body.particles[k].acceleration += force;
            }
        }
        self.body.update(
            dt,
            self.goal,
            self.heading,
            p.stiffness,
            motion.rest,
            self.behavior.time,
            &self.terrain,
        );
        for i in 0..8 {
            if !self.legs[i].moving {
                physics::limit_reach(
                    &mut self.body.particles[Body::hip_index(i)],
                    self.legs[i].planted,
                    1.04,
                );
            }
            self.legs[i].solve(self.body.hip(i));
        }
        self.web
            .update(dt, self.behavior.mode == 3, self.behavior.time);
    }
    // Versioned flat render snapshot. JavaScript reads it without owning simulation.
    pub fn render(&mut self) {
        self.frame.clear();
        let color = visual::color(self.behavior.time, self.curiosity, self.tension);
        let edge_count = (self.web.revealed as usize).min(self.web.edges.len());
        self.frame.extend_from_slice(&[
            1.0,
            self.behavior.time,
            self.heading,
            self.behavior.mode as f32,
            self.body.particles.len() as f32,
            8.0,
            edge_count as f32,
            self.behavior.affection,
            color[0],
            color[1],
            color[2],
            color[3],
            self.goal.x,
            self.goal.y,
            self.web.revealed / self.web.edges.len().max(1) as f32,
            0.0,
        ]);
        for p in &self.body.particles {
            self.frame.extend_from_slice(&[p.p.x, p.p.y, p.p.z]);
        }
        for l in &self.legs {
            for q in l.joints {
                self.frame.extend_from_slice(&[q.x, q.y, q.z])
            }
            self.frame
                .extend_from_slice(&[if l.moving { 1.0 } else { 0.0 }, l.progress]);
        }
        for e in self.web.edges.iter().take(edge_count) {
            for i in [e.a, e.b] {
                let q = self.web.nodes[i].p;
                self.frame.extend_from_slice(&[q.x, q.y, q.z]);
            }
        }
    }
}
// Single-instance ABI for this pet. Native users can own multiple Spider values.
#[cfg(target_arch = "wasm32")]
mod wasm {
    use super::*;
    use std::cell::RefCell;
    thread_local! {static PET:RefCell<Spider>=RefCell::new(Spider::new());}
    #[no_mangle]
    pub extern "C" fn pet_update(dt: f32) {
        PET.with(|p| p.borrow_mut().update(dt));
    }
    #[no_mangle]
    pub extern "C" fn pet_mode(mode: u32) {
        PET.with(|p| p.borrow_mut().set_mode(mode));
    }
    #[no_mangle]
    pub extern "C" fn pet_target(x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            PET.with(|p| p.borrow_mut().behavior.pointer = V::new(x, y, 0.0));
        }
    }
    #[no_mangle]
    pub extern "C" fn pet_environment(curiosity: f32, tension: f32, terrain: u32) {
        if curiosity.is_finite() && tension.is_finite() {
            PET.with(|p| {
                let mut s = p.borrow_mut();
                s.curiosity = curiosity.clamp(0.0, 1.0);
                s.tension = tension.clamp(0.0, 1.0);
                s.terrain.uneven = terrain != 0;
            });
        }
    }
    #[no_mangle]
    pub extern "C" fn pet_bounds(x: f32, y: f32) {
        if x.is_finite() && y.is_finite() {
            PET.with(|p| p.borrow_mut().behavior.bounds = V::new(x.max(0.5), y.max(0.5), 0.0));
        }
    }
    #[no_mangle]
    pub extern "C" fn pet_touch() {
        PET.with(|p| p.borrow_mut().behavior.affection = 1.0);
    }
    #[no_mangle]
    pub extern "C" fn pet_clear_web() {
        PET.with(|p| p.borrow_mut().web = Default::default());
    }
    #[no_mangle]
    pub extern "C" fn pet_reset() {
        PET.with(|p| *p.borrow_mut() = Spider::new());
    }
    #[no_mangle]
    pub extern "C" fn pet_frame() -> *const f32 {
        PET.with(|p| p.borrow().frame.as_ptr())
    }
    #[no_mangle]
    pub extern "C" fn pet_frame_len() -> usize {
        PET.with(|p| p.borrow().frame.len())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn constraint_respects_mass_and_pins() {
        use physics::*;
        let mut p = vec![
            Particle::new(V::default(), 0.0),
            Particle::new(V::new(2.0, 0.0, 0.0), 1.0),
        ];
        Distance {
            a: 0,
            b: 1,
            rest: 1.0,
            stiffness: 1.0,
        }
        .solve(&mut p);
        assert_eq!(p[0].p, V::default());
        assert!((p[1].p.x - 1.0).abs() < 1e-6);
        p[0].acceleration.x = 20.0;
        p[0].integrate(0.1);
        assert_eq!(p[0].p, V::default());
    }
    #[test]
    fn fabrik_preserves_lengths_and_reaches() {
        for target in [V::new(0.5, 0.2, 0.0), V::new(3.0, 0.0, 0.0), V::default()] {
            let mut j = [
                V::default(),
                V::new(0.0, 0.0, 0.3),
                V::new(0.4, 0.0, 0.4),
                V::new(0.8, 0.0, 0.0),
            ];
            ik::solve(V::default(), target, &[0.2, 0.4, 0.5], &mut j);
            for i in 0..3 {
                assert!(((j[i + 1] - j[i]).len() - [0.2, 0.4, 0.5][i]).abs() < 1e-5)
            }
            if target.len() < 1.1 {
                assert!((j[3] - target).len() < 0.002)
            }
        }
    }
    #[test]
    fn planted_feet_push_back() {
        let f = physics::tension(V::new(1.0, 0.0, 0.0), V::default(), 0.6, 100.0);
        assert!(f.x < 0.0);
        assert_eq!(
            physics::tension(V::new(0.3, 0.0, 0.0), V::default(), 0.6, 100.0),
            V::default()
        );
    }
    #[test]
    fn long_walk_stays_finite_and_keeps_support() {
        let mut s = Spider::new();
        s.terrain.uneven = true;
        s.set_mode(1);
        let mut steps = 0;
        for frame in 0..7200 {
            s.behavior.pointer = V::new(
                (frame as f32 * 0.002).sin() * 2.0,
                (frame as f32 * 0.003).cos() * 1.5,
                0.0,
            );
            let before: Vec<_> = s.legs.iter().map(|l| (l.planted, l.moving)).collect();
            s.update(1.0 / 60.0);
            assert!(s.frame.iter().all(|x| x.is_finite()));
            assert!(s.legs.iter().filter(|l| l.moving).count() <= 4);
            for (i, l) in s.legs.iter().enumerate() {
                if !before[i].1 && !l.moving {
                    assert_eq!(before[i].0, l.planted);
                    assert!(
                        (l.joints[3] - l.planted).len() < 0.02,
                        "planted contact error {} at frame {} leg {}",
                        (l.joints[3] - l.planted).len(),
                        frame,
                        i
                    )
                }
                if l.moving {
                    steps += 1
                }
                for k in 0..3 {
                    assert!(
                        ((l.joints[k + 1] - l.joints[k]).len() - [0.16, 0.43, 0.5][k]).abs()
                            < 0.0001
                    )
                }
            }
        }
        assert!(steps > 100);
    }
    #[test]
    fn web_completes_and_reset_is_clean() {
        let mut s = Spider::new();
        s.set_mode(3);
        for _ in 0..1200 {
            s.update(1.0 / 60.0)
        }
        assert_eq!(s.web.revealed as usize, s.web.edges.len());
        assert!(s.web.nodes.iter().all(|p| p.p.finite()));
        assert!(s.frame.len() > 500);
        s.set_mode(2);
        s.update(f32::NAN);
        assert!(s.frame.iter().all(|v| v.is_finite()));
    }
    #[test]
    fn frame_partition_is_deterministic() {
        let mut a = Spider::new();
        let mut b = Spider::new();
        for _ in 0..600 {
            a.update(1.0 / 60.0)
        }
        for _ in 0..1200 {
            b.update(1.0 / 120.0)
        }
        assert!((a.body.particles[0].p - b.body.particles[0].p).len() < 0.0001);
    }
}
