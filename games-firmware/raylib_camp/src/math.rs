//! Small vector and scalar math helpers for game logic.

use core::ops::{Add, Div, Mul, Sub};

/// A two-dimensional floating point vector.
///
/// Game objects are modelled in floating point for smooth motion; they are
/// converted to integer pixel coordinates only at draw time.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
pub struct Vec2 {
    /// Horizontal component.
    pub x: f32,
    /// Vertical component.
    pub y: f32,
}

impl Vec2 {
    /// Constructs a vector from its horizontal and vertical components.
    pub const fn new(x: f32, y: f32) -> Self {
        Vec2 { x, y }
    }

    /// Returns the zero vector.
    pub const fn zero() -> Self {
        Vec2 { x: 0.0, y: 0.0 }
    }

    /// Returns the squared length, which avoids a square root.
    pub fn length_squared(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Returns the Euclidean length.
    pub fn length(self) -> f32 {
        crate::math::sqrt(self.length_squared())
    }

    /// Returns a unit vector in the same direction, or the zero vector if
    /// this vector has no length.
    pub fn normalized(self) -> Self {
        let length = self.length();
        if length == 0.0 {
            Vec2::zero()
        } else {
            Vec2::new(self.x / length, self.y / length)
        }
    }
}

impl Add for Vec2 {
    type Output = Vec2;

    fn add(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x + rhs.x, self.y + rhs.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;

    fn sub(self, rhs: Vec2) -> Vec2 {
        Vec2::new(self.x - rhs.x, self.y - rhs.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;

    fn mul(self, scalar: f32) -> Vec2 {
        Vec2::new(self.x * scalar, self.y * scalar)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;

    fn div(self, scalar: f32) -> Vec2 {
        Vec2::new(self.x / scalar, self.y / scalar)
    }
}

/// Computes the square root of a non-negative value by Newton's method.
///
/// The standard library and `libm` provide `f32::sqrt`, but neither is
/// available in this allocation-free `no_std` crate, so the primitive is
/// reimplemented here using a bit-hack initial guess and a few iterations.
pub fn sqrt(value: f32) -> f32 {
    if value <= 0.0 {
        return 0.0;
    }
    let guess_bits = (value.to_bits() + (127 << 23)) >> 1;
    let mut approximation = f32::from_bits(guess_bits);
    for _ in 0..4 {
        approximation = 0.5 * (approximation + value / approximation);
    }
    approximation
}

#[cfg(test)]
mod tests {
    use super::Vec2;

    #[test]
    fn arithmetic_operates_component_wise() {
        let a = Vec2::new(2.0, 4.0);
        let b = Vec2::new(3.0, 1.0);
        assert_eq!(a + b, Vec2::new(5.0, 5.0));
        assert_eq!(a - b, Vec2::new(-1.0, 3.0));
        assert_eq!(a * 2.0, Vec2::new(4.0, 8.0));
        assert_eq!(a / 2.0, Vec2::new(1.0, 2.0));
    }

    #[test]
    fn normalization_produces_a_unit_vector() {
        let vector = Vec2::new(3.0, 4.0);
        let unit = vector.normalized();
        assert!((unit.length() - 1.0).abs() < 1e-5);
    }

    #[test]
    fn zero_vector_stays_zero_when_normalized() {
        assert_eq!(Vec2::zero().normalized(), Vec2::zero());
    }
}
