use core::num;

use crate::yin;

const WINDOW_LEN_S: f32 = 0.08;
const MAX_PERIOD_S: f32 = 0.065;

const YIN_TROUGH_THRESHOLD: f32 = 0.24;

const NOTE_NAMES: [&str; 12] = [
    "C ", "C#", "D ", "D#", "E ", "F ", "F#", "G ", "G#", "A ", "A#", "B ",
];

const C0_FREQ: f32 = 16.351597831287414;

fn display_gui(
    ui: &mut egui::Ui,
    has_pitch: bool,
    freq: f32,
    err_cents: f32,
    note_class_name: &str,
    note_octave: isize,
) {
    use egui::{Align, Color32, FontFamily, Layout, RichText};

    let col = if has_pitch {
        Color32::from_rgb(60, 60, 60)
    } else {
        Color32::from_rgb(255, 0, 0)
    };

    ui.add_space(16.);

    ui.vertical_centered(|ui| {
        ui.label(
            RichText::new("NO PITCH")
                .family(FontFamily::Monospace)
                .color(col)
                .size(18.),
        );
    });

    ui.horizontal_centered(|ui| {
        ui.with_layout(Layout::top_down(Align::Min).with_main_justify(true), |ui| {
            ui.label(
                RichText::new(format!("{:>10.2} Hz", freq))
                    .family(FontFamily::Monospace)
                    .color(Color32::from_rgb(0, 255, 0))
                    .size(24.),
            )
        });

        ui.separator();

        ui.with_layout(
            Layout::top_down(Align::Min).with_main_justify(true),
            |ui| {
                ui.label(
                    RichText::new(format!("{note_class_name}{note_octave:>2}"))
                        .family(FontFamily::Monospace)
                        .color(Color32::YELLOW)
                        .size(40.),
                )
            },
        );

        ui.separator();

        ui.with_layout(Layout::top_down(Align::Min).with_main_justify(true), |ui| {
            ui.label(
                RichText::new(format!(
                    " {}{:>5.2} cts",
                    if err_cents >= 0. { "+" } else { "-" },
                    err_cents.abs()
                ))
                .family(FontFamily::Monospace)
                .color(Color32::YELLOW)
                .size(24.),
            )
        });
    });
}

pub struct EditorState {
    yin: yin::Yin,
    planner: realfft::RealFftPlanner<f32>,
    rx: rtrb::Consumer<f32>,
    last_freq: f32,
    has_pitch: bool,
}

impl EditorState {
    #[inline]
    pub fn new(mut planner: realfft::RealFftPlanner<f32>, rx: rtrb::Consumer<f32>) -> Self {
        Self {
            yin: yin::Yin::new(&mut planner, 0, num::NonZeroUsize::MIN),
            planner,
            rx,
            last_freq: 440.,
            has_pitch: false,
        }
    }

    #[inline]
    fn get_gui_elements(&mut self, sr_value: f32) -> (bool, f32, f32, &str, isize) {
        let chunk = self.rx.read_chunk(self.rx.slots()).unwrap();

        let max_period_f = sr_value * MAX_PERIOD_S;
        let max_period = num::NonZeroUsize::new((max_period_f as usize).strict_add(1)).unwrap();
        let window_len_f = sr_value * WINDOW_LEN_S;
        let window_len = window_len_f as usize;

        if self.yin.get_params() != (window_len, max_period) {
            self.yin = yin::Yin::new(&mut self.planner, window_len, max_period);
        }

        if let Some((period, unused_lags)) = self
            .yin
            .process(chunk.as_slices().into(), YIN_TROUGH_THRESHOLD)
        {
            period.inspect(|&tau| self.last_freq = sr_value / tau);
            self.has_pitch = period.is_some();

            chunk.commit(unused_lags);
        }

        let octaves = f32::log2(self.last_freq / C0_FREQ);
        let semitones = octaves * 12.;
        let closest_note = semitones.round_ties_even();
        let err_cents = (semitones - closest_note) * 100.;
        let note_class_idx = (closest_note as isize).rem_euclid(12) as usize;
        let note_class_name = NOTE_NAMES[note_class_idx];
        let note_octave = (closest_note as isize).div_euclid(12);

        (
            self.has_pitch,
            self.last_freq,
            err_cents,
            note_class_name,
            note_octave,
        )
    }

    #[inline]
    pub fn ui(&mut self, ui: &mut egui::Ui, sr_value: f32) {
        let (has_pitch, freq, err_cents, note_class_name, note_octave) =
            self.get_gui_elements(sr_value);

        display_gui(ui, has_pitch, freq, err_cents, note_class_name, note_octave);
    }
}
