use std::{fs::File, io::Read};

use buzzer::Buzzer;
use cpu::CPU;
use keypad::Keypad;
use pixels::{Pixels, SurfaceTexture};
use rodio::OutputStream;
use winit::{
    dpi::PhysicalSize,
    event::{Event, VirtualKeyCode},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use winit_input_helper::WinitInputHelper;

mod bus;
mod buzzer;
mod cpu;
mod keypad;

const WIDTH: u32 = 64;
const HEIGHT: u32 = 32;

struct Chip8 {
    cpu: CPU,
    keypad: Keypad,
}

impl Chip8 {
    fn new() -> Self {
        Chip8 {
            cpu: CPU::new(),
            keypad: Keypad::new(),
        }
    }

    fn start(&mut self, rom_data: Vec<u8>) {
        self.cpu.load_rom(rom_data);
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

    fn tick(&mut self, input: &WinitInputHelper, buzzer: &mut Buzzer) {
        self.keypad.read(input, self.cpu.get_keypad_bus());
        buzzer.update(self.cpu.get_sound_timer() > 0);

        self.cpu.cycle();
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

    let mut input = WinitInputHelper::new();
    let (_stream, stream_handle) = OutputStream::try_default().unwrap();

    let event_loop = EventLoop::new();
    let window = {
        let size = PhysicalSize::new(WIDTH as f64 * 10.0, HEIGHT as f64 * 10.0);

        WindowBuilder::new()
            .with_title("chip-8 by ganitzsh")
            .with_inner_size(size)
            .with_min_inner_size(size)
            .with_max_inner_size(size)
            .with_resizable(false)
            .build(&event_loop)
            .unwrap()
    };

    let mut pixels = {
        let window_size = window.inner_size();
        let surface_texture = SurfaceTexture::new(window_size.width, window_size.height, &window);
        Pixels::new(WIDTH as u32, HEIGHT as u32, surface_texture).unwrap()
    };

    let mut chip8 = Chip8::new();
    let mut buzzer = Buzzer::new(&stream_handle);

    chip8.start(rom_data);

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
        }

        chip8.tick(&input, &mut buzzer);

        window.request_redraw();
    });
}
