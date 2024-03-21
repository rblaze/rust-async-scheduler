#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Ticks(u64);

impl Ticks {
    pub const MIN: Self = Ticks(u64::MIN);
    pub const MAX: Self = Ticks(u64::MAX);

    pub fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    pub fn ticks(&self) -> u64 {
        self.0
    }
}

impl core::fmt::Display for Ticks {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{} tick(s)", self.0)
    }
}

impl core::ops::Add for Ticks {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for Ticks {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}
