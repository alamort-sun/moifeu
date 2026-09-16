use crate::math::V;
#[derive(Clone, Copy)]
pub struct Surface {
    pub position: V,
    pub normal: V,
}
pub trait TerrainSampler {
    fn sample(&self, x: f32, y: f32) -> Surface;
}
#[derive(Default)]
pub struct Terrain {
    pub uneven: bool,
}
pub const STONES: [(f32, f32, f32); 3] = [(-1.8, -0.6, 0.65), (1.7, 0.9, 0.8), (0.5, -1.8, 0.5)];
impl Terrain {
    pub fn height(&self, x: f32, y: f32) -> f32 {
        if !self.uneven {
            return 0.0;
        }
        STONES
            .iter()
            .map(|&(a, b, r)| {
                let d = ((x - a).powi(2) + (y - b).powi(2)) / (r * r);
                0.22 * (1.0 - d).max(0.0).powi(2)
            })
            .sum()
    }
}
impl TerrainSampler for Terrain {
    fn sample(&self, x: f32, y: f32) -> Surface {
        let e = 0.001;
        Surface {
            position: V::new(x, y, self.height(x, y)),
            normal: V::new(
                (self.height(x - e, y) - self.height(x + e, y)) / (2.0 * e),
                (self.height(x, y - e) - self.height(x, y + e)) / (2.0 * e),
                1.0,
            )
            .unit(),
        }
    }
}
