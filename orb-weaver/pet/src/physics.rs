use crate::math::V;
#[derive(Clone, Debug)]
pub struct Particle {
    pub p: V,
    pub previous: V,
    pub acceleration: V,
    pub inverse_mass: f32,
}
impl Particle {
    pub fn new(p: V, inverse_mass: f32) -> Self {
        Self {
            p,
            previous: p,
            acceleration: V::default(),
            inverse_mass,
        }
    }
    pub fn integrate(&mut self, dt: f32) {
        if self.inverse_mass > 0.0 {
            let old = self.p;
            self.p += (self.p - self.previous) * 0.91 + self.acceleration * (dt * dt);
            self.previous = old;
        }
        self.acceleration = V::default();
    }
}
#[derive(Clone, Debug)]
pub struct Distance {
    pub a: usize,
    pub b: usize,
    pub rest: f32,
    pub stiffness: f32,
}
impl Distance {
    pub fn between(a: usize, b: usize, ps: &[Particle], stiffness: f32) -> Self {
        Self {
            a,
            b,
            rest: (ps[b].p - ps[a].p).len(),
            stiffness,
        }
    }
    pub fn solve(&self, ps: &mut [Particle]) {
        let delta = ps[self.b].p - ps[self.a].p;
        let length = delta.len();
        let wa = ps[self.a].inverse_mass;
        let wb = ps[self.b].inverse_mass;
        if length < 1e-8 || wa + wb == 0.0 {
            return;
        }
        let correction = delta * ((length - self.rest) / length * self.stiffness / (wa + wb));
        ps[self.a].p += correction * wa;
        ps[self.b].p -= correction * wb;
    }
}
// A planted foot pulls an overextended attachment back toward its world anchor.
pub fn tension(hip: V, foot: V, rest: f32, strength: f32) -> V {
    let d = foot - hip;
    d.unit() * ((d.len() - rest).max(0.0) * strength)
}

// Reach constraint used after body solving: a planted anchor may not be dragged
// beyond its chain's reach. The attachment moves, never the world contact.
pub fn limit_reach(particle: &mut Particle, anchor: V, maximum: f32) {
    let delta = particle.p - anchor;
    if particle.inverse_mass > 0.0 && delta.len() > maximum {
        particle.p = anchor + delta.unit() * maximum;
    }
}
