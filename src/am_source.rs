use rusb::UsbContext;
use rusb::Context;
use rusb::DeviceHandle;
use std::time::Duration;

const ITEMSIZE: usize = 4;
const NUM_SAMPLES: usize = 2048;

const VENDOR_ID: u16 = 0x16d0;
const PRODUCT_ID: u16 = 0x0f3b;

pub struct AmaltheaDataSupplier {
    // USB device handle
    handle: DeviceHandle<Context>,
    // Number of segments that are averaged
    averaging: u32,
    // Precomputed scale factor for mean calculation
    averaging_inv: f32,
    // Tuning frequency
    frequency: u32,
    // Sample rate
    samplerate: u32,
    // Analog bandwidth
    bandwidth: f64,
    // Buffer for receiving raw USB data
    buffer_u8: [u8; ITEMSIZE*NUM_SAMPLES],
    buffer_f32: [f32; 2*NUM_SAMPLES],
    // Buffer for computing squared magnitude of FFT output
    buffer_magsq: [f32; NUM_SAMPLES],
}

impl AmaltheaDataSupplier {


    pub fn new(averaging: u32) -> Self {

        let context = rusb::Context::new().expect("Error creating context");
    
        // Grab a reference to our device...
        let mut handle = context.open_device_with_vid_pid(VENDOR_ID, PRODUCT_ID).unwrap();

        // ... and claim its bulk interface.
        handle.claim_interface(0).expect("Error claiming interface");

        // LVDS mode
        handle.write_control(rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR, 0, 0x16, 0xa, &[], Duration::ZERO).unwrap();

        // Disable AGC
        handle.write_control(rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR, 0, 0x0, 0x20B, &[], Duration::ZERO).unwrap();
        
        //
        let averaging_inv = 1.0 / averaging as f32;

        let buffer_u8 = [0; ITEMSIZE*NUM_SAMPLES];
        let buffer_f32 = [0.0; 2*NUM_SAMPLES];
        let buffer_magsq = [0.0; NUM_SAMPLES];

        let frequency = 0;
        let samplerate = 0;
        let bandwidth = 0.0;

        Self {
            handle,
            averaging,
            averaging_inv,
            frequency,
            samplerate,
            bandwidth,
            buffer_u8,
            buffer_f32,
            buffer_magsq,
        }
    }

    pub fn write_reg(&mut self, reg: u16, value: u16) {
        self.handle.write_control(rusb::constants::LIBUSB_REQUEST_TYPE_VENDOR as u8, 0, reg, value, &[], Duration::ZERO).unwrap();
    }

    pub fn set_frequency(&mut self, frequency: u32) {
        self.frequency = frequency;
        let freq_mhz = frequency / 1000000;
        let ccf0 = ((freq_mhz - 1500) as f32 / 0.025) as u16;
        self.write_reg(ccf0 & 0xff, 0x205);
        self.write_reg((ccf0 >> 8) & 0xff, 0x206);
        self.write_reg(0x0, 0x208);
    }

    pub fn set_samplerate(&mut self, samplerate: u32) {

        // Configurable sample rates allowed by the AT86RF215 transceiver
        let samplerates: [u32; 8] = [4000000, 2000000, 4000000/3, 1000000, 800000, 2000000/3, 500000, 400000];
        let sr_values: [u16; 8] = [0x1, 0x2, 0x3, 0x4, 0x5, 0x6, 0x8, 0xA];

        let mut best_match = 0;
        let mut lowest_err = u32::MAX;
        for i in 0..8 {
            let err: u32 = (samplerate as i32 - samplerates[i] as i32).abs() as u32;
            if err < lowest_err {
                best_match = i;
                lowest_err = err;
            }
        }
        
        let sr_value: u16 = sr_values[best_match];
        self.samplerate = samplerates[best_match];

        self.write_reg((0x80 as u16) | sr_value, 0x20A);
        println!("Sample rate set to {}", self.samplerate);
    }

    pub fn set_bandwidth(&mut self, bandwidth: u32) {
        self.bandwidth = bandwidth as f64;
        println!("Bandwidth set to {}", self.bandwidth);
    }

    pub fn activate(&mut self) {
        // 24 -> TXPREP
        self.write_reg(0x3, 0x0203);
        // 24 -> RX
        self.write_reg(0x5, 0x0203);
    }

    pub fn get_block(&mut self) -> &[f32] {

        // Reset output buffer
        self.buffer_magsq.iter_mut().for_each(|x| *x = 0.0);

        for _ in 0..self.averaging {

            // Read new chunk of data
            self.handle.read_bulk(0x81, &mut self.buffer_u8, Duration::from_secs(10)).unwrap();

            // Cast as i16
            let buffer_i16: [i16; 2*NUM_SAMPLES] = unsafe { std::mem::transmute(self.buffer_u8) };

            // Convert to f32
            for i in 0..2*NUM_SAMPLES {
                self.buffer_f32[i] = (buffer_i16[i] as f32) / 32768.0;
            }

            // Cast as complex32 (layout compatible)
            let buffer_c32: [num::Complex<f32>; NUM_SAMPLES] = unsafe { std::mem::transmute(self.buffer_f32) };
            
            // Convert FFT bins to squared magnitude and add to averaging buffer
            for i in 0..NUM_SAMPLES {
                self.buffer_magsq[i] += buffer_c32[i].norm_sqr();
            }
        }

        // Scale due to averaging
        self.buffer_magsq
            .iter_mut()
            .for_each(|x| *x *= self.averaging_inv as f32);

        &self.buffer_magsq
    }
}
