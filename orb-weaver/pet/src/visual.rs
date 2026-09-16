// Two anchor palettes travel around a curved color path as the environment changes.
pub fn color(time: f32, curiosity: f32, tension: f32) -> [f32; 4] {
    let phase = time * 0.10 + curiosity * 0.7;
    let mix = (phase.sin() + 1.0) * 0.5;
    let bend = phase.cos() * phase.sin() * 0.12 * (0.5 + tension);
    [
        0.38 + 0.40 * mix,
        0.65 - 0.28 * mix + bend,
        0.82 + 0.12 * (1.0 - mix),
        0.35 + 0.2 * tension,
    ]
}
