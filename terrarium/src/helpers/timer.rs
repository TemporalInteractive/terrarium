use std::time::SystemTime;

#[derive(Clone, Debug)]
pub struct Timer {
    start: SystemTime,
}

impl Default for Timer {
    fn default() -> Self {
        Timer::new()
    }
}

impl Timer {
    pub fn new() -> Self {
        Timer {
            start: SystemTime::now(),
        }
    }

    pub fn elapsed(&self) -> f32 {
        self.start.elapsed().unwrap().as_secs_f32()
    }

    pub fn reset(&mut self) {
        self.start = SystemTime::now();
    }
}

pub struct FpsCounter {
    timer: Timer,
    frame_count: u32,
    fps: u32,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            timer: Timer::new(),
            frame_count: 0,
            fps: 0,
        }
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn end_frame(&mut self) {
        self.frame_count += 1;

        if self.timer.elapsed() >= 1.0 {
            self.fps = self.frame_count;
            self.frame_count = 0;
            self.timer.reset();
        }
    }
}
