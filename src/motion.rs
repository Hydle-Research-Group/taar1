use core::f32;
use micromath::F32Ext;

/// Generates a half-period cosecant curve, where it is defined mathematically as:
///
/// ```latex
/// f(x) = csc(x)
///
/// where x > 0 and x < π
/// ```
///
/// - `x` the value between 0 and π (if x <= 0 or x >= π, the returned value is always 10)
///
/// The returned `u32` represents the rounded value of `f(x)`.
pub fn sin_profile(x: f32) -> u32 {
    if x >= f32::consts::PI || x <= 0.0 {
        return 10; // default to 10
    }

    (1.0 / x.sin()).round() as u32
}
