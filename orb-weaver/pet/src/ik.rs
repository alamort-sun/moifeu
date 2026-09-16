use crate::math::V;
// Arbitrary-length FABRIK: targets are supplied by the caller.
pub fn solve(root: V, target: V, lengths: &[f32], joints: &mut [V]) {
    assert_eq!(joints.len(), lengths.len() + 1);
    let reach: f32 = lengths.iter().sum();
    if (target - root).len() >= reach {
        let dir = (target - root).unit();
        joints[0] = root;
        for i in 0..lengths.len() {
            joints[i + 1] = joints[i] + dir * lengths[i];
        }
        return;
    }
    let last = joints.len() - 1;
    for _ in 0..32 {
        joints[last] = target;
        for i in (0..last).rev() {
            let mut d = (joints[i] - joints[i + 1]).unit();
            if d.len() < 0.5 {
                d = V::new(0.0, 0.0, 1.0)
            }
            joints[i] = joints[i + 1] + d * lengths[i];
        }
        joints[0] = root;
        for i in 0..last {
            let mut d = (joints[i + 1] - joints[i]).unit();
            if d.len() < 0.5 {
                d = V::new(0.0, 0.0, 1.0)
            }
            joints[i + 1] = joints[i] + d * lengths[i];
        }
        if (joints[last] - target).len() < 0.0001 {
            break;
        }
    }
}
