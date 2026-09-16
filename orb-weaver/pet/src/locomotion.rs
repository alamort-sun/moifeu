use crate::{ik, math::V, world::TerrainSampler};
#[derive(Clone)]
pub struct Leg {
    pub joints: [V; 4],
    pub planted: V,
    pub desired: V,
    pub target: V,
    pub normal: V,
    pub progress: f32,
    start: V,
    pub moving: bool,
    pub group: usize,
}
impl Leg {
    pub fn new(root: V, foot: V, group: usize) -> Self {
        let mut leg = Self {
            joints: [
                root,
                root + V::new(0.08, 0.0, 0.15),
                root.lerp(foot, 0.6) + V::new(0.0, 0.0, 0.4),
                foot,
            ],
            planted: foot,
            desired: foot,
            target: foot,
            normal: V::new(0.0, 0.0, 1.0),
            progress: 1.0,
            start: foot,
            moving: false,
            group,
        };
        leg.solve(root);
        leg
    }
    pub fn solve(&mut self, root: V) {
        ik::solve(root, self.target, &[0.16, 0.43, 0.5], &mut self.joints)
    }
    pub fn begin(&mut self) {
        self.start = self.planted;
        self.progress = 0.0;
        self.moving = true;
    }
    pub fn update(&mut self, dt: f32, duration: f32, height: f32) {
        if self.moving {
            self.progress = (self.progress + dt / duration).min(1.0);
            self.target = step_curve(self.start, self.desired, self.progress, height);
            if self.progress >= 1.0 {
                self.moving = false;
                self.planted = self.desired;
                self.target = self.planted;
            }
        } else {
            self.target = self.planted
        }
    }
}
pub fn step_curve(a: V, b: V, t: f32, height: f32) -> V {
    let t = t.clamp(0.0, 1.0);
    a.lerp(b, t * t * (3.0 - 2.0 * t))
        + V::new(0.0, 0.0, (std::f32::consts::PI * t).sin().max(0.0) * height)
}
pub fn foot_target(
    center: V,
    heading: f32,
    index: usize,
    spread: f32,
    velocity: V,
    terrain: &impl TerrainSampler,
) -> (V, V) {
    let side = if index < 4 { -1.0 } else { 1.0 };
    let row = index % 4;
    let x = [0.62, 0.8, 0.8, 0.64][row] * side * spread;
    let y = [-0.73, -0.3, 0.26, 0.68][row] * spread;
    let q = center + V::new(x, y, 0.0).rotate(heading) + velocity * 0.18;
    let s = terrain.sample(q.x, q.y);
    (s.position, s.normal)
}
#[derive(Default)]
pub struct Gait {
    next: usize,
}
impl Gait {
    pub fn request(&mut self, legs: &mut [Leg], threshold: f32) {
        if legs.iter().any(|l| l.moving) {
            return;
        }
        for group in [self.next, 1 - self.next] {
            if legs
                .iter()
                .any(|l| l.group == group && (l.desired - l.planted).len() > threshold)
            {
                for leg in legs.iter_mut().filter(|l| l.group == group) {
                    leg.begin()
                }
                self.next = 1 - group;
                break;
            }
        }
    }
}
