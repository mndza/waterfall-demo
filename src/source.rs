use num::complex::Complex32;
use num::Zero;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

const NUM_SAMPLES: usize = 2048;

pub struct DataSupplier {
    // Number of segments that are averaged
    averaging: u32,
    // Precomputed scale factor for mean calculation
    averaging_inv: f32,
    // SoapySDR device
    device: soapysdr::Device,
    // Tuning frequency
    frequency: f64,
    // FFT instance optimized for our desired size
    fft: Arc<dyn Fft<f32>>,
    // Source of input samples
    rx_stream: soapysdr::RxStream<Complex32>,
    // Buffer for receiving samples
    buffer_c32: [Complex32; NUM_SAMPLES],
    // Scratch memory for FFT, avoids per-loop allocation
    scratch: [Complex32; NUM_SAMPLES],
    // Buffer for computing squared magnitude of FFT output
    buffer_magsq: [f32; NUM_SAMPLES],
}

impl DataSupplier {
    pub fn new(frequency: u32, averaging: u32) -> Self {
        let averaging_inv = 1.0 / averaging as f32;
        let fft = FftPlanner::new().plan_fft_forward(NUM_SAMPLES);

        let buffer_c32 = [Complex32::zero(); NUM_SAMPLES];
        let scratch = [Complex32::zero(); NUM_SAMPLES];
        let buffer_magsq = [f32::zero(); NUM_SAMPLES];

        let args: soapysdr::Args = "driver=hackrf".into();
        let device = soapysdr::Device::new(args).expect("No SoapySDR device found");

        let frequency = frequency as f64;
        let tuning_args: soapysdr::Args = "OFFSET=50e3".into();
        device
            .set_frequency(soapysdr::Direction::Rx, 0, frequency, tuning_args)
            .expect("Cannot set frequency");

        device
            .set_sample_rate(soapysdr::Direction::Rx, 0, 2.0e6)
            .expect("Cannot set sample rate");

        device
            .set_gain(soapysdr::Direction::Rx, 0, 30.0)
            .expect("Cannot set gain");

        let mut rx_stream: soapysdr::RxStream<Complex32> = device.rx_stream(&[0]).unwrap();

        rx_stream
            .activate(None)
            .expect("Cannot activate reception stream");

        Self {
            averaging,
            averaging_inv,
            device,
            frequency,
            fft,
            rx_stream,
            buffer_c32,
            scratch,
            buffer_magsq,
        }
    }

    pub fn set_frequency(&mut self, freq: u32) {
        self.frequency = freq as f64;
        let tuning_args: soapysdr::Args = "OFFSET=50e3".into();
        self.device
            .set_frequency(soapysdr::Direction::Rx, 0, self.frequency, tuning_args)
            .expect("Cannot set frequency");
        println!("Frequency set to {}", self.frequency);
    }

    pub fn get_block(&mut self) -> &[f32] {
        // Reset output buffer
        self.buffer_magsq.iter_mut().for_each(|x| *x = 0.0);

        for _ in 0..self.averaging {
            // Read new chunk of data
            let _num_samps = self
                .rx_stream
                .read(&[&mut self.buffer_c32], 5000000)
                .expect("unable to read stream");

            // Compute in-place FFT with scratch memory to avoid allocations
            self.fft
                .process_with_scratch(&mut self.buffer_c32, &mut self.scratch);

            // Convert FFT bins to squared magnitude and add to averaging buffer
            for i in 0..NUM_SAMPLES {
                self.buffer_magsq[i] += self.buffer_c32[i].norm_sqr();
            }
        }

        // Scale due to averaging
        self.buffer_magsq
            .iter_mut()
            .for_each(|x| *x *= self.averaging_inv as f32);

        &self.buffer_magsq
    }
}
