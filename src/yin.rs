use super::util;
use core::{cmp, iter, num};
use std::sync::Arc;

use realfft::num_complex::Complex;

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
        let fft_len = util::ThreeSmooth::new().find(|&n| n >= fft_len).unwrap();
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
    #[allow(unused)]
    pub fn fft_len(&self) -> usize {
        self.window_len
            .strict_mul(2)
            .strict_add(self.lag_sup.get().strict_sub(1))
    }

    #[inline(always)]
    pub fn fft_len_padded(&self) -> usize {
        self.autocorr_scratch.len()
    }

    #[inline(always)]
    pub fn get_params(&self) -> (usize, num::NonZeroUsize) {
        (self.window_len, self.lag_sup)
    }

    #[inline]
    fn calculate_autocorr(&mut self, signal: bislice::BiSlice<f32>) -> &mut [f32] {
        let fft_len = self.fft_len_padded();
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

        bislice::DoubleSliceMut::from_single(ref_out)
            .copy_from_slice(reference)
            .unwrap();

        self.fft.process_with_scratch(aus, rs, ts).unwrap();

        let (sig_out, sig_zeroed) = aus.split_at_mut_checked(sig_len).unwrap();

        sig_zeroed.fill(0.);

        bislice::DoubleSliceMut::from_single(sig_out)
            .copy_from_slice(sig)
            .unwrap();

        self.fft.process_with_scratch(aus, ss, ts).unwrap();

        for (ref_bin, sig_bin) in iter::zip(&mut *rs, &*ss) {
            // ref_bin = ref_bin * sig_bin.conj()
            let re = ref_bin.re.mul_add(sig_bin.re, ref_bin.im * sig_bin.im);
            let im = ref_bin.im.mul_add(sig_bin.re, -ref_bin.re * sig_bin.im);
            *ref_bin = Complex::new(re, im);
        }

        self.ifft.process_with_scratch(rs, aus, ts).unwrap();

        &mut aus[fft_len.strict_sub(max_lag)..]
    }

    #[inline]
    pub fn process(
        &mut self,
        signal: bislice::BiSlice<f32>,
        threshold: f32,
    ) -> Option<(Option<f32>, usize)> {
        let Some(in_lags) = signal.len().checked_sub(self.window_len) else {
            return None;
        };

        let (lagged, first_window) = signal.split_at(in_lags).unwrap();

        let unused_lags = in_lags.saturating_sub(self.lag_sup.get().strict_sub(1));

        let init_e = first_window.iter().fold(0., |s, &x| x.mul_add(x, s));

        let e_lost = signal.iter().rev();
        let e_gained = lagged.iter().rev();

        let energies = e_lost.zip(e_gained).scan(init_e, |e, (&l, &g)| {
            let ne = l.mul_add(-l, g.mul_add(g, *e));
            *e = ne;
            Some(ne)
        });

        let fft_len = self.fft.len() as f32;
        let autocorr_scale = -0.5 / (fft_len * init_e.sqrt());
        let autocorr_slice = self.calculate_autocorr(signal);

        let mut autocorrs = autocorr_slice.iter();
        autocorrs.next(); // skip lag 0

        let diffs = energies
            .zip(autocorrs)
            .map(move |(e, a)| autocorr_scale.mul_add(a / e.sqrt(), 0.5));

        let mut diffs_sdiffs = diffs.scan(0., |acc, x| {
            let sd = x + *acc;
            *acc = sd;
            Some((x, sd))
        });

        // d(k-2)
        let mut dm2 = 0.;
        // d(k-1)
        let mut dm1 = diffs_sdiffs.next().map(|(d, _)| d).unwrap_or(0.); // skip lag 1

        // d'(k-2)
        let mut dpm2 = 1.;
        // d'(k-1)
        let mut dpm1 = 1.;

        let mut tau = 1.;

        let mut p = None;

        for (d, sd) in diffs_sdiffs {

            let next_tau = tau + 1.;
            let dp = next_tau * d / sd;

            if dpm1 < threshold && dpm2 > dpm1 && dp > dpm1 {
                // We need a better interpolation scheme than this.
                let est = tau + util::parabolic_argmin(dm2, dm1, d);
                p = Some(est);
                break;
            }

            dpm2 = dpm1;
            dpm1 = dp;
            dm2 = dm1;
            dm1 = d;
            tau = next_tau;
        }

        Some((p, unused_lags))
    }
}
