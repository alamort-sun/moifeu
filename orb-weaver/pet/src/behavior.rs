use crate::math::V;
#[derive(Default)]
pub struct Behavior {
    pub mode: u32,
    pub pointer: V,
    pub time: f32,
    pub bounds: V,
    pub affection: f32,
}
pub struct Motion {
    pub velocity: V,
    pub rest: bool,
}
impl Behavior {
    pub fn update(&mut self, dt: f32, center: V) -> Motion {
        self.time += dt;
        self.affection = (self.affection - dt * 0.35).max(0.0);
        let target = match self.mode {
            1 => self.pointer,
            2 | 3 => center,
            _ => V::new(
                (self.time * 0.14).sin() * self.bounds.x * 0.52,
                (self.time * 0.19).sin() * self.bounds.y * 0.48,
                0.0,
            ),
        };
        let d = V::new(target.x - center.x, target.y - center.y, 0.0);
        Motion {
            velocity: if d.len() > 0.16 {
                d.unit() * (d.len() * 1.4).min(0.62)
            } else {
                V::default()
            },
            rest: self.mode == 2,
        }
    }
}
pub struct Parameters {
    pub stiffness: f32,
    pub spread: f32,
    pub duration: f32,
    pub lift: f32,
}
pub fn personality(curiosity: f32, tension: f32, rest: bool) -> Parameters {
    Parameters {
        stiffness: 0.65 + 0.3 * tension,
        spread: if rest { 0.68 } else { 1.0 + 0.08 * tension },
        duration: 0.34 - 0.1 * curiosity,
        lift: if rest { 0.07 } else { 0.13 + 0.07 * curiosity },
    }
}
