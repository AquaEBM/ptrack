use core::{num, sync::atomic::Ordering};
use egui::Color32;
use nice_plug::prelude::*;
use std::sync::Arc;

mod bislice;
mod yin;

struct Sender {
    tx: rtrb::Producer<f32>,
    editor_state: Arc<nice_plug_egui::EguiState>,
}

struct Tuner {
    sender: Option<Sender>,
    sr: Arc<AtomicF32>,
}

impl Default for Tuner {
    fn default() -> Self {
        Self {
            sender: None,
            sr: Arc::new(AtomicF32::new(44100.)),
        }
    }
}

impl Plugin for Tuner {
    const NAME: &'static str = "Pitch Tracker";
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
        // We have no parameters
        #[derive(Params)]
        struct TunerParams {}

        Arc::new(TunerParams {})
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
        if let Some(sender) = &mut self.sender
            && sender.editor_state.is_open()
        {
            let _ = sender
                .tx
                .push_partial_slice(buffer.as_slice().get(0).expect("INVALID PROCESS BUFFER"));
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        // TODO: move all this to another file

        const WINDOW_LEN_S: f32 = 0.08;
        const MAX_PERIOD_S: f32 = 0.065;

        const GUI_WINDOW_LEN_WIDTH: u32 = 415;
        const GUI_WINDOW_LEN_HEIGHT: u32 = 100;

        const YIN_TROUGH_THRESHOLD: f32 = 0.2;
        // AKA a maximum frequency of Fs / MIN_PERIOD_SAMPLES.
        const MIN_PERIOD_SAMPLES: f32 = 2.;

        const NOTE_NAMES: [&str; 12] = [
            "C ", "C#", "D ", "D#", "E ", "F ", "F#", "G ", "G#", "A ", "A#", "B ",
        ];

        const C0_FREQ: f32 = 16.351597831287414;

        let (tx, rx) = rtrb::RingBuffer::<f32>::new(16384);

        let sender = Sender {
            tx,
            editor_state: nice_plug_egui::EguiState::from_size(
                GUI_WINDOW_LEN_WIDTH,
                GUI_WINDOW_LEN_HEIGHT,
            ),
        };

        let editor_state = Arc::clone(&sender.editor_state);
        self.sender = Some(sender);
        let sr = Arc::clone(&self.sr);
        let mut planner = realfft::RealFftPlanner::<f32>::new();

        nice_plug_egui::create_egui_editor(
            editor_state,
            (
                yin::Yin::new(&mut planner, 0, num::NonZeroUsize::MIN),
                planner,
                rx,
                440.0,
                false,
            ),
            Default::default(),
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

                if let Some((period, unused_lags)) = yin.process(slice, YIN_TROUGH_THRESHOLD) {
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

                ui.add_space(15.);

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

impl Vst3Plugin for Tuner {
    const VST3_CLASS_ID: [u8; 16] = *b"aebmpitchtracker";

    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] =
        &[Vst3SubCategory::Tools, Vst3SubCategory::Analyzer];
}

nice_export_clap!(Tuner);
nice_export_vst3!(Tuner);
