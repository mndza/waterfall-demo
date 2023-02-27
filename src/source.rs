use num::complex::Complex32;
use num::Zero;
use rustfft::{Fft, FftPlanner};
use std::io::{self, BufReader, Read};
use std::sync::Arc;

const NUM_SAMPLES: usize = 2048;

pub struct DataSupplier {
    // Number of segments that are averaged
    averaging: u32,
    // FFT instance optimized for our desired size
    fft: Arc<dyn Fft<f32>>,
    // Source of input samples
    reader: BufReader<io::Stdin>,
    // Buffer to read raw samples from HackRF
    buffer: [u8; 2 * NUM_SAMPLES],
    // Buffer for conversion from i8 to Complex32
    buffer_c32: [Complex32; NUM_SAMPLES],
    // Scratch memory for FFT, avoids per-loop allocation
    scratch: [Complex32; NUM_SAMPLES],
    // Buffer for computing squared magnitude of FFT output
    buffer_magsq: [f32; NUM_SAMPLES],
}

impl DataSupplier {
    pub fn new(averaging: u32) -> Self {
        let fft = FftPlanner::new().plan_fft_forward(NUM_SAMPLES);
        let reader = BufReader::new(io::stdin());
        let buffer = [0; 2 * NUM_SAMPLES];
        let buffer_c32 = [Complex32::zero(); NUM_SAMPLES];
        let scratch = [Complex32::zero(); NUM_SAMPLES];
        let buffer_magsq = [f32::zero(); NUM_SAMPLES];
        Self {
            averaging,
            fft,
            reader,
            buffer,
            buffer_c32,
            scratch,
            buffer_magsq,
        }
    }

    pub fn get_block(&mut self) -> &[f32] {
        // Reset output buffer
        self.buffer_magsq.iter_mut().for_each(|x| *x = 0.0);

        for _ in 0..self.averaging {
            // Read new of data
            self.reader
                .read_exact(&mut self.buffer)
                .expect("error reading samples");

            // Convert samples to complex floats
            for i in 0..NUM_SAMPLES {
                self.buffer_c32[i] = Complex32 {
                    re: (self.buffer[2 * i] as i8) as f32,
                    im: (self.buffer[2 * i + 1] as i8) as f32,
                } / 128.0;
            }

            // Compute in-place FFT with scratch memory to avoid allocations
            self.fft
                .process_with_scratch(&mut self.buffer_c32, &mut self.scratch);

            // Convert FFT output to squared magnitude and add to averaging buffer
            for i in 0..NUM_SAMPLES {
                self.buffer_magsq[i] += self.buffer_c32[i].re * self.buffer_c32[i].re
                    + self.buffer_c32[i].im * self.buffer_c32[i].im;
            }
        }

        // Scale due to averaging
        let averaging_inv = 1.0 / self.averaging as f32;
        self.buffer_magsq
            .iter_mut()
            .for_each(|x| *x *= averaging_inv as f32);

        &self.buffer_magsq
    }
}
