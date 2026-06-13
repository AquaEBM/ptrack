use core::{num, sync::atomic::Ordering};
use egui::Color32;
use nice_plug::prelude::*;
use std::sync::Arc;

mod bislice;
mod yin;

struct Tuner {
    params: Arc<TunerParams>,
    tx: rtrb::Producer<f32>,
    sr: Arc<AtomicF32>,
    rx: Option<rtrb::Consumer<f32>>,
}

impl Default for Tuner {
    fn default() -> Self {
        let (tx, rx) = rtrb::RingBuffer::new(24000);
        Self {
            params: Arc::new(TunerParams::default()),
            tx,
            sr: Arc::new(AtomicF32::new(44100.)),
            rx: Some(rx),
        }
    }
}

#[derive(Params)]
struct TunerParams {
    #[persist = "editor_state"]
    editor_state: Arc<nice_plug_egui::EguiState>,
}

impl Default for TunerParams {
    fn default() -> Self {
        Self {
            editor_state: nice_plug_egui::EguiState::from_size(400, 75),
        }
    }
}

impl Plugin for Tuner {
    const NAME: &'static str = "TOP 10 PITCH TRACKER MOMENTOS";
    const VENDOR: &'static str = "AquaEBM";
    const URL: &'static str = "www.github.com/AquaEBM";
    const EMAIL: &'static str = "AquaEBM@gmail.com";

    const VERSION: &'static str = "0.0.1";

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[AudioIOLayout {
        main_input_channels: NonZeroU32::new(1),
        main_output_channels: NonZeroU32::new(1),
        ..AudioIOLayout::const_default()
    }];

    const MIDI_INPUT: MidiConfig = MidiConfig::None;
    const SAMPLE_ACCURATE_AUTOMATION: bool = false;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        self.sr.store(buffer_config.sample_rate, Ordering::Relaxed);
        true
    }

    fn process(
        &mut self,
        buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        _context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {

        let _ = self.tx.push_partial_slice(buffer.as_slice()[0]);

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // TODO: move all this to another file

        const WINDOW_LEN_S: f32 = 0.09;
        const MAX_PERIOD_S: f32 = 0.03;
        const THRESHOLD: f32 = 0.2;
        // AKA a maximum frequency of Fs / MIN_PERIOD_SAMPLES.
        const MIN_PERIOD_SAMPLES: f32 = 2.;

        const NOTE_NAMES: [&str; 12] = [
            "C ", "C#", "D ", "D#", "E ", "F ", "F#", "G ", "G#", "A ", "A#", "B ",
        ];

        const C0_FREQ: f32 = 16.351597831287414;

        let sr = Arc::clone(&self.sr);
        let mut planner = realfft::RealFftPlanner::<f32>::new();

        nice_plug_egui::create_egui_editor(
            Arc::clone(&self.params.editor_state),
            (
                yin::Yin::new(&mut planner, 0, num::NonZeroUsize::MIN),
                planner,
                self.rx.take().expect("ERROR"),
                440.0,
                false,
            ),
            nice_plug_egui::EguiSettings::default(),
            |_, _, _| {},
            move |ui, _setter, _queue, (yin, planner, rx, last_freq, has_pitch)| {
                let slots = rx.slots();
                let chunk = rx.read_chunk(slots).unwrap();
                let slice: bislice::DoubleSlice<_> = chunk.as_slices().into();

                let sr_value = sr.load(Ordering::Relaxed);
                let max_period_f = sr_value * MAX_PERIOD_S;
                let max_period =
                    num::NonZeroUsize::new((max_period_f as usize).strict_add(1)).unwrap();
                let window_len_f = sr_value * WINDOW_LEN_S;
                let window_len = window_len_f as usize;

                if yin.get_params() != (window_len, max_period) {
                    *yin = yin::Yin::new(planner, window_len, max_period);
                }

                if let Some((period, unused_lags)) = yin.process(slice, THRESHOLD) {
                    if let Some(tau) = period.filter(|&p| p > MIN_PERIOD_SAMPLES) {
                        *last_freq = sr_value / tau;
                        *has_pitch = true;
                    } else {
                        *has_pitch = false;
                    }

                    chunk.commit(unused_lags);
                }

                let ratio = f32::log2(*last_freq / C0_FREQ);
                let semitones = ratio * 12.;
                let closest_note = semitones.round_ties_even();
                let err_cents = (semitones - closest_note) * 100.;
                let note_class_idx = (closest_note as usize).rem_euclid(12);
                let note_class_name = NOTE_NAMES[note_class_idx];
                let note_octave = (closest_note as isize).div_euclid(12);

                let col = if *has_pitch {
                    Color32::from_rgb(60, 60, 60)
                } else {
                    Color32::from_rgb(255, 0, 0)
                };

                ui.vertical_centered(|ui| {
                    ui.label(egui::RichText::new("NO PITCH").color(col).size(20.));
                });

                ui.horizontal(|ui| {
                    ui.add_sized(
                        [130.0, 44.0],
                        egui::Label::new(
                            egui::RichText::new(format!("{:^9.2} Hz", *last_freq))
                                .color(Color32::from_rgb(0, 255, 0))
                                .size(20.),
                        ),
                    );
                    ui.separator();
                    ui.add_sized(
                        [110.0, 52.0],
                        egui::Label::new(
                            egui::RichText::new(format!("{note_class_name}{note_octave:<2}"))
                                .color(Color32::YELLOW)
                                .size(40.),
                        ),
                    );
                    ui.separator();
                    ui.add_sized(
                        [130.0, 44.0],
                        egui::Label::new(
                            egui::RichText::new(format!("({:>+4.2} cts)", err_cents))
                                .color(Color32::YELLOW)
                                .size(20.),
                        ),
                    );
                });
            },
        )
    }
}

impl ClapPlugin for Tuner {
    const CLAP_ID: &'static str = "com.AquaEBM.ptrack";
    const CLAP_DESCRIPTION: Option<&'static str> = Some("A Simple Pitch Tracker");
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::NoteDetector];
}

nice_export_clap!(Tuner);
