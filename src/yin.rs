use core::{cmp, iter, num};
use std::sync::Arc;

use realfft::num_complex::Complex;

pub struct ThreeSmooth {
    s: Vec<usize>,
    i2: usize,
    i3: usize,
}

impl ThreeSmooth {
    #[inline]
    pub fn new() -> Self {
        Self {
            s: vec![1],
            i2: 0,
            i3: 0,
        }
    }
}

impl Iterator for ThreeSmooth {
    type Item = usize;

    #[inline]
    fn next(&mut self) -> Option<Self::Item> {
        let current = *self.s.last().unwrap();

        let n2 = 2 * self.s[self.i2];
        let n3 = 3 * self.s[self.i3];

        let (push, add) = if n2 <= n3 {
            (n2, &mut self.i2)
        } else {
            (n3, &mut self.i3)
        };

        self.s.push(push);
        *add += 1;

        Some(current)
    }
}

pub struct Yin {
    fft: Arc<dyn realfft::RealToComplex<f32>>,
    ifft: Arc<dyn realfft::ComplexToReal<f32>>,
    window_len: usize,
    lag_sup: num::NonZeroUsize,
    ref_spec: Box<[Complex<f32>]>,
    sig_spec: Box<[Complex<f32>]>,
    scratch: Box<[Complex<f32>]>,
    autocorr_scratch: Box<[f32]>,
}

impl Yin {
    #[inline]
    pub fn new(
        planner: &mut realfft::RealFftPlanner<f32>,
        window_len: usize,
        lag_sup: num::NonZeroUsize,
    ) -> Self {
        let fft_len = Self::length_fft(window_len, lag_sup);
        let fft_len = ThreeSmooth::new().find(|&n| n >= fft_len).unwrap();
        let fft = planner.plan_fft_forward(fft_len);
        let ifft = planner.plan_fft_inverse(fft_len);
        let scratch_len = cmp::max(fft.get_scratch_len(), ifft.get_scratch_len());
        let spec_len = fft.complex_len();

        Self {
            fft,
            ifft,
            window_len,
            lag_sup,
            ref_spec: vec![Complex::ZERO; spec_len].into_boxed_slice(),
            sig_spec: vec![Complex::ZERO; spec_len].into_boxed_slice(),
            scratch: vec![Complex::ZERO; scratch_len].into_boxed_slice(),
            autocorr_scratch: vec![0.; fft_len].into_boxed_slice(),
        }
    }

    #[inline(always)]
    fn length_fft(window_len: usize, lag_sup: num::NonZeroUsize) -> usize {
        window_len
            .strict_mul(2)
            .strict_add(lag_sup.get().strict_sub(1))
    }

    #[inline(always)]
    pub fn fft_len(&self) -> usize {
        self.autocorr_scratch.len()
    }

    #[inline(always)]
    pub fn get_params(&self) -> (usize, num::NonZeroUsize) {
        (self.window_len, self.lag_sup)
    }

    #[inline]
    pub fn calculate_autocorr(&mut self, signal: super::bislice::DoubleSlice<f32>) -> &mut [f32] {
        let fft_len = self.fft_len();
        let w = self.window_len;
        let rs = self.ref_spec.as_mut();
        let ss = self.sig_spec.as_mut();
        let aus = self.autocorr_scratch.as_mut();
        let ts = self.scratch.as_mut();

        let Some(in_max_lag) = signal.len().checked_sub(w) else {
            return &mut [];
        };

        let max_lag = cmp::min(in_max_lag, self.lag_sup.get().strict_sub(1));

        let sig_len = w.strict_add(max_lag);

        let sig = signal.slice(signal.len().strict_sub(sig_len)..).unwrap();

        let reference = sig.slice(max_lag..).unwrap();

        let (ref_out, ref_zeroed) = aus.split_at_mut_checked(w).unwrap();

        ref_zeroed.fill(0.);

        super::bislice::DoubleSliceMut::from_single(ref_out)
            .copy_from_slice(reference)
            .unwrap();

        self.fft.process_with_scratch(aus, rs, ts).unwrap();

        let (sig_out, sig_zeroed) = aus.split_at_mut_checked(sig_len).unwrap();

        sig_zeroed.fill(0.);

        super::bislice::DoubleSliceMut::from_single(sig_out)
            .copy_from_slice(sig)
            .unwrap();

        self.fft.process_with_scratch(aus, ss, ts).unwrap();

        for (ref_bin, sig_bin) in iter::zip(&mut *rs, &*ss) {
            // ref_bin = ref_bin * sig_bin.conj()
            let re = f32::mul_add(ref_bin.re, sig_bin.re, ref_bin.im * sig_bin.im);
            let im = f32::mul_add(ref_bin.im, sig_bin.re, -ref_bin.re * sig_bin.im);
            *ref_bin = Complex::new(re, im);
        }

        self.ifft.process_with_scratch(rs, aus, ts).unwrap();

        &mut aus[fft_len.strict_sub(max_lag)..]
    }

    #[inline]
    pub fn process(
        &mut self,
        signal: super::bislice::DoubleSlice<f32>,
        threshold: f32,
    ) -> Option<(Option<f32>, usize)> {
        let Some(in_lags) = signal.len().checked_sub(self.window_len) else {
            return None;
        };

        let (lagged, first_window) = signal.split_at(in_lags).unwrap();

        let unused_lags = in_lags.saturating_sub(self.lag_sup.get().strict_sub(1));

        // sum x_i^2 + sum x_{i+tau}^2
        let mut current_energy = 2.
            * first_window
                .iter()
                .fold(0.0, |acc, &x| f32::mul_add(x, x, acc));

        let mut e_lost = signal.iter().rev();
        let mut e_gained = lagged.iter().rev();

        let mut cumsum = 0f32;
        let mut current_lag = 0u32;
        let mut prev_n_diff = 1f32;
        let mut prev_prev_n_diff = 1f32;

        let autocorr_scale = -2. / self.fft.len() as f32;

        let autocorr = self.calculate_autocorr(signal);

        let mut autocorrs = autocorr.iter();
        autocorrs.next(); // skip lag 0

        for &autocorr in autocorrs {
            let &lost = e_lost.next().unwrap();
            let &gained = e_gained.next().unwrap();

            current_lag = current_lag.strict_add(1);

            current_energy =
                f32::mul_add(-lost, lost, f32::mul_add(gained, gained, current_energy));

            let diff = f32::mul_add(autocorr, autocorr_scale, current_energy);

            cumsum += diff;

            let current_lag_f = current_lag as f32;

            let norm_diff = diff * current_lag_f / cumsum.max(1e-6);

            if prev_n_diff < threshold && prev_prev_n_diff > prev_n_diff && norm_diff > prev_n_diff
            {
                let y0 = prev_prev_n_diff;
                let y1 = prev_n_diff;
                let y2 = norm_diff;
                let a = (y0 + y2) / 2. - y1;
                let b = (y2 - y0) / 2.;
                if f32::abs(a) > 1e-6 {
                    let tau = current_lag_f - 1. - b / (2. * a);
                    return Some((Some(tau), unused_lags));
                }
            }

            prev_prev_n_diff = prev_n_diff;
            prev_n_diff = norm_diff;
        }

        Some((None, unused_lags))
    }
}
