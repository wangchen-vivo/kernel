// Copyright (c) 2026 vivo Mobile Communication Co., Ltd.
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//       http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! QMI8658 6-axis IMU (accelerometer + gyroscope + temperature) integration.
//!
//! The Waveshare ESP32-C6 Touch AMOLED board connects the QMI8658 at I2C
//! address 0x6B (secondary, SA0 pulled low).  This module wraps the vendor
//! `qmi8658` crate and registers `/dev/qmi86580` as a character device
//! returning a 14-byte binary report.

use blueos_driver::i2c::I2cConfig;
use embedded_hal::delay::DelayNs;
use embedded_io::ErrorKind;
use qmi8658::command::register::acceleration::{
    AccelerationOutput, AngularRateOutput,
};
use qmi8658::command::register::ctrl1::{Ctrl1Register, IntDirection};
use qmi8658::command::register::ctrl2::{AccelerometerFS, AccelerometerODR, Ctrl2Register};
use qmi8658::command::register::ctrl3::{Ctrl3Register, GyroscopeFS};
use qmi8658::command::register::ctrl5::Ctrl5Register;
use qmi8658::command::register::ctrl7::Ctrl7Register;
use qmi8658::Qmi8658;

use crate::{
    devices::{
        bus::{Bus, BusWrapper},
        i2c_core::block_i2c::BlockI2c,
        Device, DeviceClass, DeviceId, DeviceManager,
    },
    drivers::{DriverModule, InitDriver, Result as DriverResult},
    sync::{KernelDelay, SpinLock},
};
use alloc::{string::String, sync::Arc};

const QMI8658_DEVICE_NAME: &str = "qmi86580";
const QMI8658_DEVICE_MAJOR: usize = 242;
const QMI8658_DEVICE_MINOR: usize = 0;

/// Binary report size returned by `/dev/qmi86580`.
///
/// Layout (little-endian, version byte at offset 0):
///   [0]  version (u8, always 1)
///   [1]  reserved
///   [2..8]  accel X, Y, Z (3 × i16 LE, raw ADC counts)
///   [8..14] gyro  X, Y, Z (3 × i16 LE, raw ADC counts)
///   [14]  temperature high (u8)
///   [15]  temperature low (u8)
///
/// Total: 16 bytes.
///
/// The application layer applies scale factors for the configured ranges:
///   accel: ±2g → 0.009576 m/s² per LSB  (2.0 / 32768.0 * 9.80665)
///   gyro:  ±512°/s → 0.015625 °/s per LSB  (512.0 / 32768.0)
///   temperature: (high + low/256) °C
pub const QMI8658_REPORT_SIZE: usize = 16;
const QMI8658_REPORT_VERSION: u8 = 1;

/// QMI8658 I2C secondary address (SA0 pulled low).
const QMI8658_I2C_ADDR: u8 = 0x6B;

/// Kernel delay adapter for the vendor crate's `DelayNs` trait.
struct Qmi8658Delay(KernelDelay);

impl DelayNs for Qmi8658Delay {
    fn delay_ns(&mut self, ns: u32) {
        self.0.delay_ns(ns);
    }

    fn delay_ms(&mut self, ms: u32) {
        self.0.delay_ms(ms);
    }
}

pub struct Qmi8658Device<T: blueos_hal::i2c::I2c<I2cConfig, ()>> {
    sensor: SpinLock<Qmi8658<BusWrapper<BlockI2c<T>>, Qmi8658Delay>>,
}

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()>> Qmi8658Device<T> {
    fn new(sensor: Qmi8658<BusWrapper<BlockI2c<T>>, Qmi8658Delay>) -> Self {
        Self {
            sensor: SpinLock::new(sensor),
        }
    }

    fn encode_report(
        accel: AccelerationOutput,
        gyro: AngularRateOutput,
        temperature: f32,
        report: &mut [u8; QMI8658_REPORT_SIZE],
    ) {
        report[0] = QMI8658_REPORT_VERSION;
        report[1] = 0; // reserved

        // Accel raw ADC counts (i16 LE)
        let ax = accel.x as i16;
        let ay = accel.y as i16;
        let az = accel.z as i16;
        report[2..4].copy_from_slice(&ax.to_le_bytes());
        report[4..6].copy_from_slice(&ay.to_le_bytes());
        report[6..8].copy_from_slice(&az.to_le_bytes());

        // Gyro raw ADC counts (i16 LE)
        let gx = gyro.x as i16;
        let gy = gyro.y as i16;
        let gz = gyro.z as i16;
        report[8..10].copy_from_slice(&gx.to_le_bytes());
        report[10..12].copy_from_slice(&gy.to_le_bytes());
        report[12..14].copy_from_slice(&gz.to_le_bytes());

        // Temperature: integer + fractional parts
        let temp_int = temperature as u8;
        let temp_frac = ((temperature - temp_int as f32) * 256.0) as u8;
        report[14] = temp_int;
        report[15] = temp_frac;
    }
}

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()>> Device for Qmi8658Device<T> {
    fn name(&self) -> String {
        String::from(QMI8658_DEVICE_NAME)
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        DeviceId::new(QMI8658_DEVICE_MAJOR, QMI8658_DEVICE_MINOR)
    }

    fn read(
        &self,
        _pos: u64,
        buf: &mut [u8],
        _is_nonblocking: bool,
    ) -> core::result::Result<usize, ErrorKind> {
        if buf.len() < QMI8658_REPORT_SIZE {
            return Err(ErrorKind::InvalidInput);
        }

        let mut sensor = self.sensor.lock();
        let accel = sensor.get_acceleration_raw().map_err(|error| {
            log::warn!("Failed to read QMI8658 acceleration: {:?}", error);
            ErrorKind::Other
        })?;
        let gyro = sensor.get_angular_rate_raw().map_err(|error| {
            log::warn!("Failed to read QMI8658 angular rate: {:?}", error);
            ErrorKind::Other
        })?;
        let temperature = sensor.get_temperature().map_err(|error| {
            log::warn!("Failed to read QMI8658 temperature: {:?}", error);
            ErrorKind::Other
        })?;

        let mut report = [0u8; QMI8658_REPORT_SIZE];
        Self::encode_report(accel, gyro, temperature, &mut report);
        buf[..QMI8658_REPORT_SIZE].copy_from_slice(&report);
        Ok(QMI8658_REPORT_SIZE)
    }

    fn write(
        &self,
        _pos: u64,
        _buf: &[u8],
        _is_nonblocking: bool,
    ) -> core::result::Result<usize, ErrorKind> {
        Err(ErrorKind::Unsupported)
    }
}

pub struct Qmi8658Config;

impl Qmi8658Config {
    pub const fn new() -> Self {
        Self
    }
}

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()>> InitDriver<BlockI2c<T>> for Qmi8658Config {
    type Data = ();

    fn init(self, bus: &Bus<BlockI2c<T>>) -> DriverResult<Self::Data> {
        let mut delay = Qmi8658Delay(KernelDelay);
        let mut sensor = qmi8658::Qmi8658::new_secondary_address(bus.intf.clone(), delay);

        // Verify device identity
        let device_id = sensor.get_device_id().map_err(|error| {
            log::warn!("Failed to read QMI8658 device ID: {:?}", error);
            crate::error::code::EIO
        })?;
        log::info!("QMI8658 device ID: {:?}", device_id);

        // Software reset
        sensor.reset().map_err(|error| {
            log::warn!("Failed to reset QMI8658: {:?}", error);
            crate::error::code::EIO
        })?;
        Qmi8658Delay(KernelDelay).delay_ms(10);

        // Configure CTRL1: enable accelerometer and gyroscope I2C interface
        let mut ctrl1 = Ctrl1Register(0);
        ctrl1.set_int1_enable(false);
        ctrl1.set_int2_enable(false);
        ctrl1.set_fifo_int_sel(IntDirection::Int1);
        sensor.set_ctrl1(ctrl1).map_err(|error| {
            log::warn!("Failed to set QMI8658 CTRL1: {:?}", error);
            crate::error::code::EIO
        })?;

        // Configure CTRL2: accelerometer ±2g, 1000 Hz ODR
        let mut ctrl2 = Ctrl2Register(0);
        ctrl2.set_afs(AccelerometerFS::FS2G);
        ctrl2.set_aodr(AccelerometerODR::NormalAODR3); // 1000 Hz
        sensor.set_ctrl2(ctrl2).map_err(|error| {
            log::warn!("Failed to set QMI8658 CTRL2: {:?}", error);
            crate::error::code::EIO
        })?;

        // Configure CTRL3: gyroscope ±512°/s, 1000 Hz ODR
        let mut ctrl3 = Ctrl3Register(0);
        ctrl3.set_gfs(GyroscopeFS::DPS512);
        sensor.set_ctrl3(ctrl3).map_err(|error| {
            log::warn!("Failed to set QMI8658 CTRL3: {:?}", error);
            crate::error::code::EIO
        })?;

        // Configure CTRL5: disable auto-increment
        let ctrl5 = Ctrl5Register(0);
        sensor.set_ctrl5(ctrl5).map_err(|error| {
            log::warn!("Failed to set QMI8658 CTRL5: {:?}", error);
            crate::error::code::EIO
        })?;

        // Configure CTRL7: enable accelerometer and gyroscope
        let mut ctrl7 = Ctrl7Register(0);
        ctrl7.set_accelerometer_enable(true);
        ctrl7.set_gyroscope_enable(true);
        sensor.set_ctrl7(ctrl7).map_err(|error| {
            log::warn!("Failed to set QMI8658 CTRL7: {:?}", error);
            crate::error::code::EIO
        })?;

        // Small delay to let sensors stabilize
        Qmi8658Delay(KernelDelay).delay_ms(10);

        let device = Arc::new(Qmi8658Device::<T>::new(sensor));
        DeviceManager::get()
            .register_device(String::from(QMI8658_DEVICE_NAME), device)
            .map_err(|_| crate::error::code::EIO)?;

        log::info!(
            "QMI8658 IMU initialized at address 0x{:X} as /dev/{}",
            QMI8658_I2C_ADDR,
            QMI8658_DEVICE_NAME
        );

        Ok(())
    }
}

pub struct Qmi8658DriverModule;

impl<T: blueos_hal::i2c::I2c<I2cConfig, ()>> DriverModule<BlockI2c<T>> for Qmi8658DriverModule {
    type Data = Qmi8658Config;

    fn probe(dev: &crate::devices::DeviceData) -> DriverResult<Self::Data> {
        match dev {
            crate::devices::DeviceData::Native(native_dev) => {
                if native_dev.is_attached() {
                    return Err(crate::error::code::ENODEV);
                }

                // QMI8658 has no configurable pins; just check that the
                // device description exists on I2C0.
                let _ = native_dev.config::<Qmi8658Config>().ok_or(crate::error::code::ENODEV)?;
                Ok(Qmi8658Config::new())
            }
            _ => Err(crate::error::code::ENODEV),
        }
    }
}