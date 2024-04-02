/// Point in time.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Instant(u64);

impl Instant {
    pub const MIN: Self = Instant(u64::MIN);
    pub const MAX: Self = Instant(u64::MAX);

    pub fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    pub fn ticks(&self) -> u64 {
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
pub struct Duration(u64);

impl Duration {
    pub const MIN: Self = Duration(u64::MIN);
    pub const MAX: Self = Duration(u64::MAX);

    pub fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    pub fn ticks(&self) -> u64 {
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
