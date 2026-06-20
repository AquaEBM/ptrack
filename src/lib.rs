use core::sync::atomic::Ordering;
use nice_plug::prelude::*;
use std::sync::Arc;

mod editor;
mod util;
mod yin;

struct Sender {
    tx: rtrb::Producer<f32>,
    editor_state: Arc<nice_plug_egui::EguiState>,
}

#[derive(Default)]
struct Tuner {
    sender: Option<Sender>,
    sr: Arc<AtomicF32>,
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
        struct TunerParams {}

        // SAFETY: empty parameter list
        unsafe impl Params for TunerParams {
            fn param_map(&self) -> Vec<(String, ParamPtr, String)> {
                vec![]
            }
        }

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
            let buf = buffer.as_slice().get(0).expect("INVALID PROCESS BUFFER");
            let _ = sender.tx.push_partial_slice(buf);
        }

        ProcessStatus::Normal
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        const GUI_WINDOW_LEN_WIDTH: u32 = 415;
        const GUI_WINDOW_LEN_HEIGHT: u32 = 100;

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

        nice_plug_egui::create_egui_editor(
            editor_state,
            editor::EditorState::new(realfft::RealFftPlanner::<f32>::new(), rx),
            Default::default(),
            |_, _, _| {},
            move |ui, _, _, state| {
                state.ui(ui, sr.load(Ordering::Relaxed));
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
