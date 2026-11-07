use std::sync::{
    Arc,
    atomic::{AtomicBool, AtomicU64, Ordering},
};

use cpal::{
    FromSample, SizedSample,
    traits::{DeviceTrait, HostTrait, StreamTrait},
};
use magic_ui_egui::StudioAudioOut;

type Fill = Box<dyn FnMut(&mut [f32], usize) + Send>;

struct Output {
    device: cpal::Device,
    config: cpal::SupportedStreamConfig,
    stream: Option<cpal::Stream>,
    frames: Arc<AtomicU64>,
    failed: Arc<AtomicBool>,
}

pub fn open() -> Result<Box<dyn StudioAudioOut>, String> {
    let device = cpal::default_host()
        .default_output_device()
        .ok_or("No default output device")?;
    let config = device
        .default_output_config()
        .map_err(|error| error.to_string())?;
    Ok(Box::new(Output {
        device,
        config,
        stream: None,
        frames: Arc::default(),
        failed: Arc::default(),
    }))
}

impl Output {
    fn stream<T: SizedSample + FromSample<f32>>(
        &self,
        mut fill: Fill,
    ) -> Result<cpal::Stream, String> {
        let channels = self.channels();
        let frames = Arc::clone(&self.frames);
        let failed = Arc::clone(&self.failed);
        let mut scratch = Vec::new();
        self.device
            .build_output_stream(
                &self.config.config(),
                move |samples: &mut [T], _| {
                    scratch.resize(samples.len(), 0.0);
                    fill(&mut scratch, channels);
                    for (dst, src) in samples.iter_mut().zip(&scratch) {
                        *dst = T::from_sample(*src);
                    }
                    frames.fetch_add((samples.len() / channels) as u64, Ordering::Relaxed);
                },
                move |_| {
                    failed.store(true, Ordering::Relaxed);
                },
                None,
            )
            .map_err(|error| error.to_string())
    }
}

impl StudioAudioOut for Output {
    fn start(&mut self, fill: Fill) -> Result<u32, String> {
        self.stop();
        self.frames.store(0, Ordering::Relaxed);
        self.failed.store(false, Ordering::Relaxed);
        let stream = match self.config.sample_format() {
            cpal::SampleFormat::F32 => self.stream::<f32>(fill),
            cpal::SampleFormat::I16 => self.stream::<i16>(fill),
            cpal::SampleFormat::U16 => self.stream::<u16>(fill),
            format => Err(format!("Unsupported Studio audio sample format {format}")),
        }?;
        stream.play().map_err(|error| error.to_string())?;
        self.stream = Some(stream);
        Ok(self.sample_rate())
    }

    fn stop(&mut self) {
        self.stream = None;
    }
    fn sample_rate(&self) -> u32 {
        self.config.sample_rate().0
    }
    fn channels(&self) -> usize {
        usize::from(self.config.channels()).max(1)
    }
    fn played_frames(&self) -> Option<u64> {
        (self.stream.is_some() && !self.failed.load(Ordering::Relaxed))
            .then(|| self.frames.load(Ordering::Relaxed))
    }
}
