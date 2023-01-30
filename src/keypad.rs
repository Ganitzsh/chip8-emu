use winit::event::VirtualKeyCode;
use winit_input_helper::WinitInputHelper;

use crate::bus::Bus;

pub struct Keypad {
    mapping: [(VirtualKeyCode, u8); 0x9],
}

impl Keypad {
    pub fn new() -> Self {
        Keypad {
            mapping: [
                (VirtualKeyCode::Key1, 0x1),
                (VirtualKeyCode::Key2, 0x2),
                (VirtualKeyCode::Key3, 0x3),
                (VirtualKeyCode::Key4, 0x4),
                (VirtualKeyCode::Key5, 0x5),
                (VirtualKeyCode::Key6, 0x6),
                (VirtualKeyCode::Key7, 0x7),
                (VirtualKeyCode::Key8, 0x8),
                (VirtualKeyCode::Key9, 0x9),
            ],
        }
    }

    pub fn read(&self, input: &WinitInputHelper, bus: &mut Bus) {
        for (key, value) in self.mapping {
            if input.key_pressed(key) {
                bus.send(value, 0x1)
            }

            if input.key_released(key) {
                bus.send(value, 0x0);
            }
        }
    }
}
