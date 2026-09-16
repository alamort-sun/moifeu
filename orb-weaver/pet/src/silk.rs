use crate::{
    math::V,
    physics::{Distance, Particle},
};
#[derive(Default)]
pub struct Web {
    pub nodes: Vec<Particle>,
    pub edges: Vec<Distance>,
    pub revealed: f32,
    pub origin: V,
}
impl Web {
    pub fn build(&mut self, origin: V) {
        self.origin = origin;
        self.nodes = vec![Particle::new(V::new(origin.x, origin.y, 0.04), 0.5)];
        self.edges.clear();
        self.revealed = 0.0;
        let n = 14;
        let rings = 7;
        for r in 1..=rings {
            for j in 0..n {
                let a = j as f32 * std::f32::consts::TAU / n as f32;
                let radius = r as f32 * 0.29;
                let p = origin + V::new(a.cos() * radius, a.sin() * radius, 0.035);
                self.nodes
                    .push(Particle::new(p, if r == rings { 0.0 } else { 1.0 }));
            }
        }
        // Radials first, then one continuous capture spiral.
        for j in 0..n {
            for r in 0..rings {
                let a = if r == 0 { 0 } else { 1 + (r - 1) * n + j };
                let b = 1 + r * n + j;
                self.edges.push(Distance::between(a, b, &self.nodes, 0.8));
            }
        }
        for i in 1..self.nodes.len() - 1 {
            self.edges
                .push(Distance::between(i, i + 1, &self.nodes, 0.6));
        }
    }
    pub fn update(&mut self, dt: f32, weaving: bool, time: f32) {
        if weaving {
            self.revealed = (self.revealed + dt * 14.0).min(self.edges.len() as f32)
        }
        if self.revealed == 0.0 {
            return;
        }
        for (i, p) in self.nodes.iter_mut().enumerate() {
            p.acceleration.z = (time * 2.0 + i as f32 * 0.3).sin() * 0.1;
            p.integrate(dt);
        }
        for _ in 0..4 {
            for e in &self.edges {
                e.solve(&mut self.nodes)
            }
        }
    }
}
