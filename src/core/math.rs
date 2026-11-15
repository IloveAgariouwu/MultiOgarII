use std::ops::{Add, AddAssign, Mul, Sub, SubAssign};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    pub x: f64,
    pub y: f64,
}

impl Vec2 {
    pub const ZERO: Vec2 = Vec2 { x: 0.0, y: 0.0 };

    pub fn new(x: f64, y: f64) -> Self {
        Self { x, y }
    }

    pub fn from_angle(angle: f64) -> Self {
        Self::new(angle.cos(), angle.sin())
    }

    pub fn angle(self) -> f64 {
        self.y.atan2(self.x)
    }

    pub fn dist_sq(self) -> f64 {
        self.x * self.x + self.y * self.y
    }

    pub fn dist(self) -> f64 {
        self.dist_sq().sqrt()
    }

    pub fn normalized(self) -> Self {
        let d = self.dist();
        if d == 0.0 {
            self
        } else {
            self * (1.0 / d)
        }
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x + o.x, self.y + o.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, o: Vec2) -> Vec2 {
        Vec2::new(self.x - o.x, self.y - o.y)
    }
}

impl Mul<f64> for Vec2 {
    type Output = Vec2;
    fn mul(self, k: f64) -> Vec2 {
        Vec2::new(self.x * k, self.y * k)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, o: Vec2) {
        self.x += o.x;
        self.y += o.y;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, o: Vec2) {
        self.x -= o.x;
        self.y -= o.y;
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub min_x: f64,
    pub min_y: f64,
    pub max_x: f64,
    pub max_y: f64,
}

impl Rect {
    pub fn new(min_x: f64, min_y: f64, max_x: f64, max_y: f64) -> Self {
        Self { min_x, min_y, max_x, max_y }
    }

    pub fn around(center: Vec2, half_w: f64, half_h: f64) -> Self {
        Self::new(center.x - half_w, center.y - half_h, center.x + half_w, center.y + half_h)
    }

    pub fn width(&self) -> f64 {
        self.max_x - self.min_x
    }

    pub fn height(&self) -> f64 {
        self.max_y - self.min_y
    }

}
