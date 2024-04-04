/// Point in time.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(i64);

impl Instant {
    pub const MIN: Self = Instant(i64::MIN);
    pub const MAX: Self = Instant(i64::MAX);

    pub fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    pub fn ticks(&self) -> i64 {
        self.0
    }
}

impl core::fmt::Display for Instant {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "tick {}", self.0)
    }
}

/// Length of time interval between two Instants.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Duration(i64);

impl Duration {
    pub const MIN: Self = Duration(i64::MIN);
    pub const MAX: Self = Duration(i64::MAX);

    pub fn new(ticks: i64) -> Self {
        Self(ticks)
    }

    pub fn ticks(&self) -> i64 {
        self.0
    }
}

impl core::fmt::Display for Duration {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == 1 {
            write!(f, "1 tick")
        } else {
            write!(f, "{} ticks", self.0)
        }
    }
}

impl core::ops::Add<Duration> for Instant {
    type Output = Self;

    fn add(self, rhs: Duration) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for Instant {
    type Output = Duration;

    fn sub(self, rhs: Self) -> Self::Output {
        Duration(self.0 - rhs.0)
    }
}

impl core::ops::Add for Duration {
    type Output = Self;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0)
    }
}

impl core::ops::Sub for Duration {
    type Output = Self;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0 - rhs.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_instant_add() {
        let a = Instant::new(10);
        let b = Duration::new(5);
        assert_eq!(a + b, Instant::new(15));
    }

    #[test]
    fn test_instant_sub() {
        let a = Instant::new(10);
        let b = Instant::new(5);
        assert_eq!(a - b, Duration::new(5));
        assert_eq!(b - a, Duration::new(-5));
    }

    #[test]
    fn test_duration_add() {
        let a = Duration::new(10);
        let b = Duration::new(5);
        assert_eq!(a + b, Duration::new(15));
    }

    #[test]
    fn test_duration_sub() {
        let a = Duration::new(10);
        let b = Duration::new(5);
        assert_eq!(a - b, Duration::new(5));
        assert_eq!(b - a, Duration::new(-5));
    }
}
