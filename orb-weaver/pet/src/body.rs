use crate::{
    math::V,
    physics::{Distance, Particle},
    world::{Terrain, TerrainSampler},
};
pub struct Body {
    pub particles: Vec<Particle>,
    pub constraints: Vec<Distance>,
    local: Vec<V>,
}
impl Body {
    pub fn new() -> Self {
        let mut local = vec![];
        for (count, rx, ry, cy) in [(8, 0.20, 0.26, -0.16), (12, 0.32, 0.40, 0.35)] {
            for i in 0..count {
                let a = i as f32 * std::f32::consts::TAU / count as f32
                    + if count == 8 {
                        std::f32::consts::PI / 8.0
                    } else {
                        0.0
                    };
                local.push(V::new(a.cos() * rx, a.sin() * ry + cy, 0.27));
            }
        }
        local.push(V::new(0.0, -0.16, 0.3));
        local.push(V::new(0.0, 0.35, 0.33));
        let particles: Vec<_> = local.iter().map(|&p| Particle::new(p, 1.0)).collect();
        let mut constraints = vec![];
        for (start, n, c) in [(0, 8, 20), (8, 12, 21)] {
            for j in 0..n {
                constraints.push(Distance::between(
                    start + j,
                    start + (j + 1) % n,
                    &particles,
                    0.8,
                ));
                constraints.push(Distance::between(start + j, c, &particles, 0.7));
                constraints.push(Distance::between(
                    start + j,
                    start + (j + n / 2) % n,
                    &particles,
                    0.4,
                ));
            }
        }
        for (a, b) in [(20, 21), (1, 9), (3, 13), (5, 15), (7, 19)] {
            constraints.push(Distance::between(a, b, &particles, 0.7))
        }
        Self {
            particles,
            constraints,
            local,
        }
    }
    pub fn center(&self) -> V {
        let p = self.particles[20].p;
        V::new(p.x, p.y + 0.16, 0.0)
    }
    pub fn hip_index(i: usize) -> usize {
        if i < 4 {
            [5, 4, 3, 2][i]
        } else {
            [6, 7, 0, 1][i - 4]
        }
    }
    pub fn hip(&self, i: usize) -> V {
        self.particles[Self::hip_index(i)].p
    }
    pub fn update(
        &mut self,
        dt: f32,
        goal: V,
        heading: f32,
        stiffness: f32,
        rest: bool,
        time: f32,
        terrain: &Terrain,
    ) {
        for (i, p) in self.particles.iter_mut().enumerate() {
            let mut target = goal + self.local[i].rotate(heading);
            target.z += terrain.height(target.x, target.y);
            if rest {
                target.z -= 0.12
            }
            target.z += (time * 2.3).sin() * 0.009;
            if i < 20 {
                target.z += (time * 2.3 + i as f32 * 0.2).sin() * 0.004
            }
            p.acceleration += (target - p.p) * 180.0;
            p.integrate(dt);
        }
        for _ in 0..8 {
            for c in &self.constraints {
                let mut c = c.clone();
                c.stiffness *= stiffness;
                c.solve(&mut self.particles);
            }
            for p in &mut self.particles {
                p.p.z = p.p.z.max(terrain.sample(p.p.x, p.p.y).position.z + 0.045)
            }
        }
    }
}
