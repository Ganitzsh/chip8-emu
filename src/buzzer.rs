use std::time::{Duration, SystemTime};

use rodio::{source::SineWave, OutputStreamHandle, Sink, Source};

const MIN_PLAYBACK_DURATION: f32 = 0.25; // 250ms

pub struct Buzzer {
    sink: Sink,
    last_started_at: SystemTime,
}

impl Buzzer {
    pub fn new(output: &OutputStreamHandle) -> Self {
        let sink = Sink::try_new(output).unwrap();

        sink.append(
            SineWave::new(440.0)
                .take_duration(Duration::from_secs_f32(10.0))
                .repeat_infinite(),
        );
        sink.pause();

        Buzzer {
            sink,
            last_started_at: SystemTime::now(),
        }
    }

    pub fn update(&mut self, is_buzzing: bool) {
        if is_buzzing && self.sink.is_paused() {
            self.sink.play();
            self.last_started_at = SystemTime::now();
            return;
        }

        if !is_buzzing
            && !self.sink.is_paused()
            && self.last_started_at.elapsed().unwrap().as_secs_f32() >= MIN_PLAYBACK_DURATION
        {
            self.sink.pause();
        }
    }
}
