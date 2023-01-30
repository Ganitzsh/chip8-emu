pub struct Bus {
    signals: Vec<(u8, u8)>,
}

impl Bus {
    pub fn new() -> Self {
        Bus {
            signals: Vec::new(),
        }
    }

    pub fn send(&mut self, d1: u8, d2: u8) {
        self.signals.push((d1, d2));
    }

    pub fn read(&mut self) -> (u8, u8) {
        match self.signals.pop() {
            Some(signal) => signal,
            None => (0x0, 0x0),
        }
    }
}
