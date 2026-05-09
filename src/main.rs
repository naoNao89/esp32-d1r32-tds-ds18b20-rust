#![no_std]
#![no_main]

//! Wemos D1 R32 (ESP32) + DS18B20 (temp) + Gravity TDS V1.0 (conductivity).
//!
//! Wiring:
//!   DS18B20      VDD -> 3V3, GND -> GND, DQ -> GPIO14 (D7), 4.7k DQ<->3V3
//!   TDS V1.0     VCC -> 3V3, GND -> GND, A  -> GPIO34 (input-only, ADC1_6)
//!
//! TDS V1.0: 3.3-5V, 0-1000 ppm, analog 0-2.3 V (powered at 3.3V the swing
//! stays under ~1.5 V which fits the ESP32 ADC at 11 dB attenuation).
//! Temperature compensation uses the DS18B20 reading.
//! Logs stream over UART0 (CH340 USB-serial) at 115200 baud.

use esp_backtrace as _;
use esp_hal::{
    analog::adc::{Adc, AdcConfig, Attenuation},
    delay::Delay,
    gpio::{DriveMode, Flex, InputConfig, OutputConfig, Pull},
    main,
};
use esp_println::println;
use nb::block;

// ---------- minimal 1-wire bit-bang ----------

struct OneWire<'a> {
    pin: Flex<'a>,
    d: Delay,
}

#[derive(Debug)]
enum OwError {
    NoPresence,
    CrcMismatch,
}

impl<'a> OneWire<'a> {
    fn new(mut pin: Flex<'a>, d: Delay) -> Self {
        pin.apply_input_config(&InputConfig::default().with_pull(Pull::Up));
        pin.apply_output_config(
            &OutputConfig::default()
                .with_drive_mode(DriveMode::OpenDrain)
                .with_pull(Pull::Up),
        );
        pin.set_input_enable(true);
        pin.set_output_enable(true);
        pin.set_high();
        Self { pin, d }
    }

    fn write_bit(&mut self, bit: bool) {
        self.pin.set_low();
        if bit {
            self.d.delay_micros(6);
            self.pin.set_high();
            self.d.delay_micros(64);
        } else {
            self.d.delay_micros(60);
            self.pin.set_high();
            self.d.delay_micros(10);
        }
    }

    fn read_bit(&mut self) -> bool {
        self.pin.set_low();
        self.d.delay_micros(6);
        self.pin.set_high();
        self.d.delay_micros(9);
        let b = self.pin.is_high();
        self.d.delay_micros(55);
        b
    }

    fn write_byte(&mut self, mut byte: u8) {
        for _ in 0..8 {
            self.write_bit(byte & 1 == 1);
            byte >>= 1;
        }
    }

    fn read_byte(&mut self) -> u8 {
        let mut b = 0u8;
        for i in 0..8 {
            if self.read_bit() {
                b |= 1 << i;
            }
        }
        b
    }

    fn reset(&mut self) -> Result<(), OwError> {
        self.pin.set_low();
        self.d.delay_micros(480);
        self.pin.set_high();
        self.d.delay_micros(70);
        let presence = self.pin.is_low();
        self.d.delay_micros(410);
        if presence { Ok(()) } else { Err(OwError::NoPresence) }
    }
}

fn crc8(data: &[u8]) -> u8 {
    let mut crc = 0u8;
    for &b in data {
        let mut x = b;
        for _ in 0..8 {
            let mix = (crc ^ x) & 0x01;
            crc >>= 1;
            if mix != 0 {
                crc ^= 0x8C;
            }
            x >>= 1;
        }
    }
    crc
}

// ---------- DS18B20 ----------

const SEARCH_ROM:     u8 = 0xF0;
const MATCH_ROM:      u8 = 0x55;
const SKIP_ROM:       u8 = 0xCC;
const CONVERT_T:      u8 = 0x44;
const READ_SCRATCHPAD: u8 = 0xBE;

/// Standard Maxim 1-Wire ROM SEARCH. Returns up to `MAX` 8-byte ROM codes.
/// See "Application Note 187" / DS18B20 datasheet "ROM SEARCH" section.
fn ow_search<const MAX: usize>(ow: &mut OneWire) -> ([[u8; 8]; MAX], usize) {
    let mut found: [[u8; 8]; MAX] = [[0u8; 8]; MAX];
    let mut count = 0;
    let mut last_discrepancy: i32 = 0;
    let mut last_device = false;
    let mut rom_no: [u8; 8] = [0; 8];

    while !last_device && count < MAX {
        if ow.reset().is_err() {
            return (found, count);
        }
        ow.write_byte(SEARCH_ROM);

        let mut id_bit_number: i32 = 1;
        let mut last_zero: i32 = 0;
        let mut rom_byte_number: usize = 0;
        let mut rom_byte_mask: u8 = 1;

        loop {
            let id_bit  = ow.read_bit();
            let cmp_bit = ow.read_bit();
            if id_bit && cmp_bit {
                // no devices responded
                return (found, count);
            }
            let search_direction: bool = if id_bit != cmp_bit {
                id_bit
            } else {
                // discrepancy
                if id_bit_number < last_discrepancy {
                    (rom_no[rom_byte_number] & rom_byte_mask) != 0
                } else {
                    id_bit_number == last_discrepancy
                }
            };
            if !id_bit && !cmp_bit && !search_direction {
                last_zero = id_bit_number;
            }
            if search_direction {
                rom_no[rom_byte_number] |= rom_byte_mask;
            } else {
                rom_no[rom_byte_number] &= !rom_byte_mask;
            }
            ow.write_bit(search_direction);

            id_bit_number += 1;
            rom_byte_mask = rom_byte_mask.wrapping_shl(1);
            if rom_byte_mask == 0 {
                rom_byte_number += 1;
                rom_byte_mask = 1;
            }
            if rom_byte_number >= 8 {
                break;
            }
        }

        if id_bit_number >= 65 {
            last_discrepancy = last_zero;
            if last_discrepancy == 0 {
                last_device = true;
            }
            // CRC check on the ROM (last byte must equal CRC of first 7).
            if crc8(&rom_no[..7]) == rom_no[7] {
                found[count] = rom_no;
                count += 1;
            }
        }
    }
    (found, count)
}

/// Read temperature from the DS18B20 with the given ROM. Issues an explicit
/// MATCH_ROM so this works on a bus with multiple sensors.
fn ds18b20_read_celsius_rom(ow: &mut OneWire, rom: &[u8; 8]) -> Result<f32, OwError> {
    ow.reset()?;
    ow.write_byte(MATCH_ROM);
    for &b in rom { ow.write_byte(b); }
    ow.write_byte(CONVERT_T);
    ow.d.delay_millis(800);

    ow.reset()?;
    ow.write_byte(MATCH_ROM);
    for &b in rom { ow.write_byte(b); }
    ow.write_byte(READ_SCRATCHPAD);

    let mut scratch = [0u8; 9];
    for s in scratch.iter_mut() { *s = ow.read_byte(); }
    if crc8(&scratch[..8]) != scratch[8] {
        return Err(OwError::CrcMismatch);
    }
    let raw = i16::from_le_bytes([scratch[0], scratch[1]]);
    Ok(raw as f32 / 16.0)
}

fn ds18b20_read_celsius(ow: &mut OneWire) -> Result<f32, OwError> {
    ow.reset()?;
    ow.write_byte(SKIP_ROM);
    ow.write_byte(CONVERT_T);
    // 12-bit conversion can take up to 750 ms
    ow.d.delay_millis(800);

    ow.reset()?;
    ow.write_byte(SKIP_ROM);
    ow.write_byte(READ_SCRATCHPAD);

    let mut scratch = [0u8; 9];
    for s in scratch.iter_mut() {
        *s = ow.read_byte();
    }
    if crc8(&scratch[..8]) != scratch[8] {
        return Err(OwError::CrcMismatch);
    }
    let raw = i16::from_le_bytes([scratch[0], scratch[1]]);
    Ok(raw as f32 / 16.0)
}

// ---------- TDS (Gravity SEN0244 / TDS V1.0) ----------

/// VCC supplied to the TDS board. The DFRobot reference firmware was
/// calibrated at 5 V. We power the board from the D1 R32 5V (USB) rail.
/// NOTE: GPIO34 is 3.3 V tolerant only — the TDS board's analog output
/// peaks around 2.3 V even on 5 V supply, which is safe for the ADC.
const TDS_VREF: f32 = 5.0;

/// ADC full scale at 11 dB attenuation on ESP32 (~0..3.3 V mapped to 0..4095).
const ADC_VREF: f32 = 3.3;
const ADC_MAX:  f32 = 4095.0;

/// Apply DFRobot's reference TDS algorithm (Gravity SEN0244).
fn tds_ppm_from_voltage(v_adc: f32, temp_c: f32) -> f32 {
    // Temperature compensation: 2 % per °C from 25 °C reference.
    let comp = 1.0 + 0.02 * (temp_c - 25.0);
    let v = v_adc / comp;
    // Cubic polynomial -> ppm (DFRobot reference).
    (133.42 * v * v * v - 255.86 * v * v + 857.39 * v) * 0.5
}

// ---------- entry ----------

#[main]
fn main() -> ! {
    let peripherals = esp_hal::init(esp_hal::Config::default());
    esp_println::logger::init_logger_from_env();

    let delay = Delay::new();

    // DS18B20 on GPIO14
    let dq = Flex::new(peripherals.GPIO14);
    let mut ow = OneWire::new(dq, delay);

    // TDS analog on GPIO34 (ADC1_6, input-only)
    let mut adc_cfg = AdcConfig::new();
    let mut tds_pin = adc_cfg.enable_pin(peripherals.GPIO34, Attenuation::_11dB);
    let mut adc1 = Adc::new(peripherals.ADC1, adc_cfg);

    println!("D1 R32 + DS18B20 (GPIO14) + TDS V1.0 (GPIO34) starting...");

    // Last good temperature, used to compensate the TDS reading.
    let mut last_temp_c: f32 = 25.0;

    // Re-scan the bus every iteration so the user can plug/unplug a DS18B20
    // at runtime and see the count + ROM list update live.
    loop {
        let (roms, n) = ow_search::<4>(&mut ow);
        println!("--- 1-Wire scan on GPIO14: found {} device(s) ---", n);
        for i in 0..n {
            let r = &roms[i];
            println!(
                "  [{}] ROM = {:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}{}",
                i, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7],
                if r[0] == 0x28 { " (DS18B20)" } else { "" }
            );
        }
        if n == 0 {
            println!("  (no devices — check wiring / 4.7k pull-up)");
        }

        // ---- 1. Temperature(s) ----
        let temp_ok = if n == 0 {
            match ds18b20_read_celsius(&mut ow) {
                Ok(c) => { last_temp_c = c; true }
                Err(_) => false,
            }
        } else {
            let mut any_ok = false;
            for i in 0..n {
                match ds18b20_read_celsius_rom(&mut ow, &roms[i]) {
                    Ok(c) => {
                        let r = &roms[i];
                        println!(
                            "  T[{}] {:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X}{:02X} = {:.3} C",
                            i, r[0], r[1], r[2], r[3], r[4], r[5], r[6], r[7], c
                        );
                        last_temp_c = c;
                        any_ok = true;
                    }
                    Err(e) => println!("  T[{}] read error: {:?}", i, e),
                }
            }
            any_ok
        };

        // 2. TDS — the board excites the probe at ~1 kHz, so sampling at
        //    just 200 Hz aliases away the peak. Sample as fast as the ADC
        //    allows for ~50 ms to be sure we catch many AC cycles, then
        //    report (a) the average of the top 1% of samples (peak), (b)
        //    median (DC offset), (c) min/max for sanity.
        const N: usize = 600;
        let mut samples = [0u16; N];
        for s in samples.iter_mut() {
            *s = block!(adc1.read_oneshot(&mut tds_pin)).unwrap();
        }
        samples.sort_unstable();
        let smin = samples[0];
        let smed = samples[N/2];
        let smax = samples[N-1];
        // average of top 6 samples (top 1%) for a stable peak estimate
        let peak_sum: u32 = samples[N-6..].iter().map(|&x| x as u32).sum();
        let raw = (peak_sum / 6) as u16;
        println!(
            "  TDS samples (n={}): min={:>4}  median={:>4}  peak_avg={:>4}  max={:>4}",
            N, smin, smed, raw, smax
        );
        let v_adc = (raw as f32) * ADC_VREF / ADC_MAX;
        let ppm = tds_ppm_from_voltage(v_adc, last_temp_c);

        if temp_ok {
            println!(
                "temp = {:.3} C | tds_raw = {:>4} | v = {:.3} V | tds = {:.1} ppm (Vsupply={:.1}V)",
                last_temp_c, raw, v_adc, ppm, TDS_VREF
            );
        } else {
            println!(
                "temp = err     | tds_raw = {:>4} | v = {:.3} V | tds = {:.1} ppm (T_assumed={:.1} C)",
                raw, v_adc, ppm, last_temp_c
            );
        }

        delay.delay_millis(1000);
    }
}
