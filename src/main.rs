use std::{
    fs::File,
    io::Read,
    time::{Duration, SystemTime},
};

use pixels::{Pixels, SurfaceTexture};
use rand::Rng;
use rodio::{source::SineWave, OutputStream, Sink, Source};
use winit::{
    dpi::LogicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

const FREQUENCY: f32 = 500.0;
const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;

struct CPU {
    registers: [u8; 0x10],
    memory: [u8; 0x1000],
    pc: u16,
    pointer: u16,
    display: [u8; WIDTH as usize * HEIGHT as usize],
    stack: Vec<u16>,
    delay_timer: u8,
    delay_timer_timestamp: SystemTime,
    sound_timer: u8,
    sound_timer_timestamp: SystemTime,
    play_sound: bool,
    key_pressed: Option<u8>,
    key_released: Option<u8>,
}

impl CPU {
    fn new() -> CPU {
        CPU {
            registers: [0; 0x10],
            memory: [0; 0x1000],
            display: [0; WIDTH as usize * HEIGHT as usize],
            stack: Vec::with_capacity(16),
            pc: 0,
            pointer: 0,
            delay_timer: 0,
            delay_timer_timestamp: SystemTime::now(),
            sound_timer: 0,
            sound_timer_timestamp: SystemTime::now(),
            play_sound: false,
            key_pressed: None,
            key_released: None,
        }
    }

    fn update_sound_timer(&mut self) {
        if self.sound_timer_timestamp.elapsed().unwrap().as_millis()
            >= ((1.0 / (FREQUENCY as f32 * 0.12)) * 1000.0) as u128
        {
            self.sound_timer_timestamp = SystemTime::now();
            self.sound_timer = self.sound_timer.saturating_sub(1);
        }

        if !self.play_sound && self.sound_timer > 0 {
            self.play_sound = true;
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

    fn cycle(&mut self) {
        let memory_opcode = self.read_memory_opcode();

        self.pc += 2;

        // println!("Opcode: {:#X}", memory_opcode);

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

        if self.key_pressed == Some(expected_key) {
            self.pc += 2;
        }
    }

    fn skip_if_key_not_pressed(&mut self, register_x: u8) {
        let expected_key = self.registers[register_x as usize];

        if self.key_pressed != Some(expected_key) {
            self.pc += 2;
        }
    }

    fn wait_for_key_press(&mut self, register_x: u8) {
        match self.key_pressed {
            Some(key) => {
                self.registers[register_x as usize] = key;
            }
            None => self.pc -= 2,
        }
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

    fn pretty_print_memory(&self) {
        for (i, byte) in self.memory.iter().enumerate() {
            if i % 16 == 0 {
                println!("");
                print!("0x{:04X} ", i);
            }

            print!("{:02X} ", byte);
        }

        println!("");
    }

    fn load_rom(&mut self, rom: Vec<u8>) {
        for (i, byte) in rom.iter().enumerate() {
            self.memory[i + 0x200] = *byte;
        }
    }

    fn set_key_pressed(&mut self, key: u8) {
        self.key_pressed = Some(key);
    }

    fn set_key_released(&mut self, key: u8) {
        self.key_pressed = None;
        self.key_released = Some(key);
    }
}

struct Chip8 {
    cpu: CPU,
}

impl Chip8 {
    fn new() -> Self {
        Chip8 { cpu: CPU::new() }
    }

    fn start(&mut self, rom_data: Vec<u8>) {
        self.cpu.load_rom(rom_data);
        self.cpu.pc = 0x200;
    }

    fn draw(&self, frame: &mut [u8]) {
        for (i, pixel) in frame.chunks_exact_mut(4).enumerate() {
            let x = (i % WIDTH as usize) as i16;
            let y = (i / WIDTH as usize) as i16;

            let chip8_pixel = self.cpu.display[(y * WIDTH as i16 + x) as usize];

            let color = if chip8_pixel == 1 {
                [0x5e, 0x48, 0xe8, 0xff]
            } else {
                [0x48, 0xb2, 0xe8, 0xff]
            };

            pixel.copy_from_slice(&color);
        }
    }
}

fn read_file(path: &str) -> Vec<u8> {
    let mut file = File::open(path).unwrap();
    let mut buffer = Vec::new();

    file.read_to_end(&mut buffer).unwrap();

    buffer
}

fn main() {
    let rom_data = read_file("roms/airplane.ch8");

    let event_loop = EventLoop::new();
    let mut input = WinitInputHelper::new();

    let window = {
        let size = LogicalSize::new(WIDTH as f64, HEIGHT as f64);
        let scaled_size = LogicalSize::new(WIDTH as f64 * 3.0, HEIGHT as f64 * 3.0);

        WindowBuilder::new()
            .with_title("chip-8")
            .with_inner_size(scaled_size)
            .with_min_inner_size(size)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture =
            SurfaceTexture::new(window_size.width * 3, window_size.height * 3, &window);
        Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture).unwrap()
    };

    let (_stream, stream_handle) = OutputStream::try_default().unwrap();
    let sink = Sink::try_new(&stream_handle).unwrap();
    sink.append(
        SineWave::new(440.0)
            .take_duration(Duration::from_secs_f32(10.0))
            .repeat_infinite(),
    );
    sink.pause();

    // Add a dummy source of the sake of the example.

    let mut chip8 = Chip8::new();

    chip8.start(rom_data);

    let keypad_mapping = [
        (VirtualKeyCode::Key1, 0x1),
        (VirtualKeyCode::Key2, 0x2),
        (VirtualKeyCode::Key3, 0x3),
        (VirtualKeyCode::Key4, 0x4),
        (VirtualKeyCode::Key5, 0x5),
        (VirtualKeyCode::Key6, 0x6),
        (VirtualKeyCode::Key7, 0x7),
        (VirtualKeyCode::Key8, 0x8),
        (VirtualKeyCode::Key9, 0x9),
    ];

    event_loop.run(move |event, _, control_flow| {
        if let Event::RedrawRequested(_) = event {
            chip8.draw(pixels.get_frame_mut());

            pixels.render().unwrap();
        }

        if input.update(&event) {
            if input.key_pressed(VirtualKeyCode::Escape) || input.quit() {
                *control_flow = ControlFlow::Exit;
                return;
            }

            for (key, value) in keypad_mapping {
                if input.key_pressed(key) {
                    chip8.cpu.set_key_pressed(value);
                }

                if input.key_released(key) {
                    chip8.cpu.set_key_released(value);
                }
            }
        }

        chip8.cpu.cycle();

        if chip8.cpu.sound_timer > 0 {
            sink.play();
        } else {
            sink.pause();
        }

        window.request_redraw();
    });
}
