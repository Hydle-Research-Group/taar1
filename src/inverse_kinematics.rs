use micromath::F32Ext;

/// Solves the angles theta and phi for base/arm rotations, where the returned value (base, arm) is in degrees.
///
/// - `x`: the x coordinate
/// - `y`: the y coordinate
/// - `z`: the z coordinate
pub fn solve(x: f32, y: f32, z: f32) -> (f32, f32) {
    let base_rotation = (y / x).atan(); // tan(θ) = y / x
    let hypotenuse_len = (x.powi(2) + y.powi(2)).sqrt(); // hypotenuse_len = sqrt(x^2 + y^2)
    let arm_rotation = (hypotenuse_len / z).atan(); // tan(Φ) = hypotenuse_len / z

    (base_rotation.to_degrees(), arm_rotation.to_degrees())
}
