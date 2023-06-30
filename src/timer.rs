use core::cell::RefCell;
use critical_section::Mutex;

struct Ticker {
    ticks: u32,
}

impl Ticker {
    // Return next tick every time it is checked
    fn get_ticks(&mut self) -> u32 {
        let ticks = self.ticks;
        self.ticks += 1;
        ticks
    }
}

struct GlobalTimer {
    ticker: Mutex<RefCell<Ticker>>,
}

impl GlobalTimer {
    const fn new() -> Self {
        Self {
            ticker: Mutex::new(RefCell::new(Ticker { ticks: 0 })),
        }
    }
}

static GLOBAL_TIMER: GlobalTimer = GlobalTimer::new();

pub fn get_ticks() -> u32 {
    critical_section::with(|cs| GLOBAL_TIMER.ticker.borrow_ref_mut(cs).get_ticks())
}
