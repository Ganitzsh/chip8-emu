use rand::Rng;
use std::time::SystemTime;

use crate::bus::Bus;

const FREQUENCY: f32 = 500.0;
const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;

pub struct CPU {
    buses: [Bus; 0x2],
    key_registers: [u8; 0x9],
    registers: [u8; 0x10],
    memory: [u8; 0x1000],
    pc: u16,
    pointer: u16,
    pub display: [u8; WIDTH as usize * HEIGHT as usize],
    stack: Vec<u16>,
    delay_timer: u8,
    delay_timer_timestamp: SystemTime,
    pub sound_timer: u8,
    sound_timer_timestamp: SystemTime,
}

impl CPU {
    pub fn new() -> CPU {
        CPU {
            buses: [Bus::new(), Bus::new()],
            key_registers: [0; 0x9],
            registers: [0; 0x10],
            memory: [0; 0x1000],
            display: [0; WIDTH as usize * HEIGHT as usize],
            stack: Vec::with_capacity(16),
            pc: 0x200,
            pointer: 0,
            delay_timer: 0,
            delay_timer_timestamp: SystemTime::now(),
            sound_timer: 0,
            sound_timer_timestamp: SystemTime::now(),
        }
    }

    pub fn get_keypad_bus(&mut self) -> &mut Bus {
        &mut self.buses[0x0]
    }

    pub fn get_sound_timer(&self) -> u8 {
        self.sound_timer
    }

    fn update_sound_timer(&mut self) {
        if self.sound_timer > 0
            && self.sound_timer_timestamp.elapsed().unwrap().as_millis()
                >= ((1.0 / (FREQUENCY as f32 * 0.12)) * 1000.0) as u128
        {
            self.sound_timer_timestamp = SystemTime::now();
            self.sound_timer = self.sound_timer.saturating_sub(1);
        }
    }

    fn update_delay_timer(&mut self) {
        if self.delay_timer_timestamp.elapsed().unwrap().as_millis()
            >= ((1.0 / (FREQUENCY as f32 * 0.12)) * 1000.0) as u128
        {
            self.delay_timer_timestamp = SystemTime::now();
            self.delay_timer = self.delay_timer.saturating_sub(1);
        }
    }

    fn read_memory_opcode(&self) -> u16 {
        let p = self.pc;

        let op_byte1 = self.memory[p as usize] as u16;
        let op_byte2 = self.memory[p as usize + 1] as u16;

        op_byte1 << 8 | op_byte2
    }

    fn decompose_opcode(&self, opcode: u16) -> (u8, u8, u8, u8) {
        let op_byte1 = (opcode >> 12) as u8;
        let op_byte2 = ((opcode >> 8) & 0x000F) as u8;
        let op_byte3 = ((opcode >> 4) & 0x000F) as u8;
        let op_byte4 = (opcode & 0x000F) as u8;

        (op_byte1, op_byte2, op_byte3, op_byte4)
    }

    pub fn cycle(&mut self) {
        let memory_opcode = self.read_memory_opcode();

        self.pc += 2;

        match self.decompose_opcode(memory_opcode) {
            (0, 0, 0xE, 0) => self.clear_display(),
            (0, 0, 0xE, 0xE) => self.return_from_subroutine(),
            (1, n1, n2, n3) => self.goto(n1, n2, n3),
            (2, n1, n2, n3) => self.call_subroutine(n1, n2, n3),
            (3, x, n1, n2) => self.skip_if_equal(x, n1, n2),
            (4, x, n1, n2) => self.skip_if_not_equal(x, n1, n2),
            (5, x, y, 0) => self.skip_if_equal_registers(x, y),
            (6, x, b1, b2) => self.set_register(x, b1 << 4 | b2),
            (7, x, b1, b2) => self.add_to_register(x, b1 << 4 | b2),
            (8, x, y, 0) => self.set_register_x_to_register_y(x, y),
            (8, x, y, 1) => self.set_register_x_or_register_y(x, y),
            (8, x, y, 2) => self.set_register_x_and_register_y(x, y),
            (8, x, y, 3) => self.set_register_x_xor_register_y(x, y),
            (8, x, y, 4) => self.add_register_y_to_register_x(x, y),
            (8, x, y, 5) => self.sub_register_y_to_register_x(x, y),
            (8, x, _, 6) => self.store_shift_register_x_least(x),
            (8, x, y, 7) => self.diff_register_y_and_register_x(x, y),
            (8, x, _, 0xE) => self.store_shift_register_x_most(x),
            (9, x, y, 0) => self.comp_register_x_register_y_skip(x, y),
            (0xA, n1, n2, n3) => self.set_pointer_address(n1, n2, n3),
            (0xB, n1, n2, n3) => self.jump_to_address_plus_v0(n1, n2, n3),
            (0xC, x, n1, n2) => self.set_register_x_rand_and_value(x, n1, n2),
            (0xD, x, y, n) => self.draw_sprite(x, y, n),
            (0xE, x, 9, 0xE) => self.skip_if_key_pressed(x),
            (0xE, x, 0xA, 1) => self.skip_if_key_not_pressed(x),
            (0xF, x, 0, 7) => self.set_register_x_to_delay_timer(x),
            (0xF, x, 0, 0xA) => self.wait_for_key_press(x),
            (0xF, x, 1, 5) => self.set_delay_timer_to_register_x(x),
            (0xF, x, 1, 8) => self.set_sound_timer_to_register_x(x),
            (0xF, x, 1, 0xE) => self.add_register_x_to_pointer(x),
            (0xF, x, 2, 9) => self.set_pointer_to_sprite(x),
            (0xF, x, 3, 3) => self.store_bcd_in_memory(x),
            (0xF, x, 5, 5) => self.store_registers_in_memory(x),
            (0xF, x, 6, 5) => self.fills_memory_from_registers(x),
            (0, 0, 0, 0) => panic!("Done"),
            _ => todo!("Unknown instruction {:04X}", memory_opcode),
        }

        self.update_delay_timer();
        self.update_sound_timer();
        self.read_keypad_bus();
    }

    fn read_keypad_bus(&mut self) {
        match self.buses[0].read() {
            (0x0, 0x0) => (),
            (key, value) => self.key_registers[key as usize] = value,
        };
    }

    fn store_bcd_in_memory(&mut self, register_x: u8) {
        let reg_value = self.registers[register_x as usize];

        let mut bcd: [u8; 3] = [0, 0, reg_value % 10];

        for (div, i) in [100, 10].iter().zip([0, 1]) {
            bcd[i] = ((reg_value - reg_value % div) / div) % div;
        }

        self.memory[self.pointer as usize..self.pointer as usize + 3].copy_from_slice(&bcd);
    }

    fn skip_if_key_pressed(&mut self, register_x: u8) {
        let expected_key = self.registers[register_x as usize];

        if self.key_registers[expected_key as usize] == 0x1 {
            self.pc += 2;
        }
    }

    fn skip_if_key_not_pressed(&mut self, register_x: u8) {
        let expected_key = self.registers[register_x as usize];

        if self.key_registers[expected_key as usize] == 0x0 {
            self.pc += 2;
        }
    }

    fn wait_for_key_press(&mut self, register_x: u8) {
        for key in 0..0x9 {
            if self.key_registers[key] == 0x1 {
                self.registers[register_x as usize] = self.key_registers[key];
                return;
            }
        }

        self.pc -= 2;
    }

    fn skip_if_equal_registers(&mut self, register_x: u8, register_y: u8) {
        if self.registers[register_x as usize] == self.registers[register_y as usize] {
            self.pc += 2;
        }
    }

    fn fills_memory_from_registers(&mut self, max_register: u8) {
        self.registers[0..max_register as usize + 1].copy_from_slice(
            &self.memory[self.pointer as usize..self.pointer as usize + max_register as usize + 1],
        );
    }

    fn store_registers_in_memory(&mut self, max_register: u8) {
        self.memory[self.pointer as usize..self.pointer as usize + max_register as usize + 1]
            .copy_from_slice(&self.registers[0..max_register as usize + 1]);
    }

    fn add_register_x_to_pointer(&mut self, register: u8) {
        self.pointer += self.registers[register as usize] as u16;
    }

    fn set_register_x_to_delay_timer(&mut self, register: u8) {
        self.registers[register as usize] = self.delay_timer;
    }

    fn set_delay_timer_to_register_x(&mut self, register: u8) {
        self.delay_timer = self.registers[register as usize];
    }

    fn set_sound_timer_to_register_x(&mut self, register: u8) {
        self.sound_timer = self.registers[register as usize];
    }

    fn set_pointer_to_sprite(&mut self, register: u8) {
        self.pointer = (self.registers[register as usize] * 5) as u16;
    }

    fn set_register(&mut self, register: u8, value: u8) {
        self.registers[register as usize] = value;
    }

    fn add_to_register(&mut self, register: u8, value: u8) {
        let (value, _) = self.registers[register as usize].overflowing_add(value);

        self.registers[register as usize] = value;
    }

    fn set_register_x_to_register_y(&mut self, register_x: u8, register_y: u8) {
        self.registers[register_x as usize] = self.registers[register_y as usize];
    }

    fn set_register_x_or_register_y(&mut self, register_x: u8, register_y: u8) {
        self.registers[register_x as usize] |= self.registers[register_y as usize];
    }

    fn set_register_x_and_register_y(&mut self, register_x: u8, register_y: u8) {
        self.registers[register_x as usize] &= self.registers[register_y as usize];
    }

    fn set_register_x_xor_register_y(&mut self, register_x: u8, register_y: u8) {
        self.registers[register_x as usize] ^= self.registers[register_y as usize];
    }

    fn add_register_y_to_register_x(&mut self, register_x: u8, register_y: u8) {
        let (v, overflow) = self.registers[register_x as usize]
            .overflowing_add(self.registers[register_y as usize]);

        if overflow {
            self.registers[0xF] = 1;
        } else {
            self.registers[0xF] = 0;
        }

        self.registers[register_x as usize] = v;
    }

    fn sub_register_y_to_register_x(&mut self, register_x: u8, register_y: u8) {
        let (v, overflow) = self.registers[register_x as usize]
            .overflowing_sub(self.registers[register_y as usize]);

        if overflow {
            self.registers[0xF] = 1;
        } else {
            self.registers[0xF] = 0;
        }

        self.registers[register_x as usize] = v;
    }

    fn store_shift_register_x_least(&mut self, register_x: u8) {
        let least_significant_bit = self.registers[register_x as usize] & 0x0F;

        self.registers[0xF] = least_significant_bit;
        self.registers[register_x as usize] >>= 1;
    }

    fn store_shift_register_x_most(&mut self, register_x: u8) {
        let most_significant_bit = (self.registers[register_x as usize] >> 7) & 1;

        self.registers[0xF] = most_significant_bit;
        self.registers[register_x as usize] <<= 1;
    }

    fn diff_register_y_and_register_x(&mut self, register_x: u8, register_y: u8) {
        let register_x_value = self.registers[register_x as usize];
        self.registers[register_x as usize] =
            self.registers[register_y as usize] - register_x_value;
    }

    fn comp_register_x_register_y_skip(&mut self, register_x: u8, register_y: u8) {
        if self.registers[register_x as usize] != self.registers[register_y as usize] {
            self.pc += 2;
        }
    }

    fn set_pointer_address(&mut self, n1: u8, n2: u8, n3: u8) {
        self.pointer = ((n1 as u16) << 8 | (n2 as u16) << 4 | n3 as u16) as u16;
    }

    fn jump_to_address_plus_v0(&mut self, n1: u8, n2: u8, n3: u8) {
        let address = ((n1 as u16) << 8 | (n2 as u16) << 4 | n3 as u16) as u16;

        self.pc = address + self.registers[0] as u16;
    }

    fn set_register_x_rand_and_value(&mut self, register_x: u8, n1: u8, n2: u8) {
        let random_number: u8 = rand::thread_rng().gen();

        self.registers[register_x as usize] = random_number & (n1 << 4 | n2);
    }

    fn goto(&mut self, n1: u8, n2: u8, n3: u8) {
        let address = ((n1 as u16) << 8 | (n2 as u16) << 4 | n3 as u16) as u16;

        self.pc = address;
    }

    fn call_subroutine(&mut self, n1: u8, n2: u8, n3: u8) {
        let address = ((n1 as u16) << 8 | (n2 as u16) << 4 | n3 as u16) as u16;

        self.stack.push(self.pc);
        self.pc = address;
    }

    fn return_from_subroutine(&mut self) {
        self.pc = self.stack.pop().unwrap();
    }

    fn skip_if_equal(&mut self, register_x: u8, n1: u8, n2: u8) {
        let value = n1 << 4 | n2;

        if self.registers[register_x as usize] == value {
            self.pc += 2;
        }
    }

    fn skip_if_not_equal(&mut self, register_x: u8, n1: u8, n2: u8) {
        let value = n1 << 4 | n2;

        if self.registers[register_x as usize] != value {
            self.pc += 2;
        }
    }

    fn clear_display(&mut self) {
        self.display = [0; WIDTH as usize * HEIGHT as usize];
    }

    fn draw_sprite(&mut self, register_x: u8, register_y: u8, n1: u8) {
        let x = self.registers[register_x as usize] as usize;
        let y = self.registers[register_y as usize] as usize;
        let height = n1 as usize;

        self.registers[0xF] = 0;

        for y_line in 0..height {
            let pixel = self.memory[self.pointer as usize + y_line];

            for x_line in 0..8 {
                if (pixel & (0x80 >> x_line)) != 0 {
                    let index = (x + x_line + ((y + y_line) * WIDTH as usize))
                        % (WIDTH as usize * HEIGHT as usize);

                    if self.display[index] == 1 {
                        self.registers[0xF] = 1;
                    }

                    self.display[index] ^= 1;
                }
            }
        }
    }

    #[allow(dead_code)]
    pub fn pretty_print_memory(&self) {
        for (i, byte) in self.memory.iter().enumerate() {
            if i % 16 == 0 {
                println!("");
                print!("0x{:04X} ", i);
            }

            print!("{:02X} ", byte);
        }

        println!("");
    }

    pub fn load_rom(&mut self, rom: Vec<u8>) {
        for (i, byte) in rom.iter().enumerate() {
            self.memory[i + 0x200] = *byte;
        }
    }
}
