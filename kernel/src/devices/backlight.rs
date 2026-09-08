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

//! Backlight brightness device (Linux backlight class semantics).
//!
//! Provides a character device `/dev/backlight` that exposes brightness control
//! via `read`/`write` of a decimal string (0-255), matching the semantics of
//! Linux sysfs `/sys/class/backlight/*/brightness`.

use crate::devices::{Device, DeviceClass, DeviceId, DeviceManager};
use alloc::{string::String, sync::Arc, vec::Vec};
use blueos_infra::tinyrwlock::RwLock;
use embedded_io::ErrorKind;

/// Backlight driver operations.
///
/// Thread-safety is guaranteed by the device wrapper: all access goes through
/// `Arc<RwLock<T>>`, mirroring the `FramebufferDevice` pattern.
pub trait BacklightOps {
    /// Set the display brightness (0 = minimum, 255 = maximum).
    fn set_brightness(&mut self, value: u8) -> Result<(), ErrorKind>;

    /// Return the current brightness value.
    fn brightness(&self) -> Result<u8, ErrorKind>;
}

/// Character device that exposes backlight brightness as a readable/writable
/// decimal string, analogous to Linux `/sys/class/backlight/*/brightness`.
pub struct BacklightDevice<T: BacklightOps> {
    name: String,
    id: DeviceId,
    ops: Arc<RwLock<T>>,
}

unsafe impl<T: BacklightOps> Send for BacklightDevice<T> {}
unsafe impl<T: BacklightOps> Sync for BacklightDevice<T> {}

impl<T: BacklightOps + 'static> BacklightDevice<T> {
    /// Create a new backlight device named `"backlight"`.
    #[must_use]
    pub fn new(ops: Arc<RwLock<T>>) -> Self {
        Self {
            name: String::from("backlight"),
            id: DeviceId::new(240, 2),
            ops,
        }
    }

    /// Register the backlight device with the device manager.
    /// The device node will appear as `/dev/backlight` after VFS init.
    pub fn register(ops: Arc<RwLock<T>>) -> Result<(), ErrorKind> {
        let device = Arc::new(Self::new(ops));
        let name = device.name.clone();
        log::debug!("Registering backlight device: {}", name);
        DeviceManager::get().register_device(name, device)
    }
}

impl<T: BacklightOps> Device for BacklightDevice<T> {
    fn name(&self) -> String {
        self.name.clone()
    }

    fn class(&self) -> DeviceClass {
        DeviceClass::Char
    }

    fn id(&self) -> DeviceId {
        self.id
    }

    fn read(&self, _pos: u64, buf: &mut [u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        if buf.is_empty() {
            return Ok(0);
        }

        let value = self.ops.read().brightness()?;
        let text = alloc::format!("{}\n", value);
        let text_bytes = text.as_bytes();
        let len = text_bytes.len().min(buf.len());
        buf[..len].copy_from_slice(&text_bytes[..len]);
        Ok(len)
    }

    fn write(&self, _pos: u64, buf: &[u8], _is_blocking: bool) -> Result<usize, ErrorKind> {
        if buf.is_empty() {
            return Ok(0);
        }

        // Parse leading decimal digits from the buffer (trim whitespace/newlines).
        let start = buf.iter().position(|b| !b.is_ascii_whitespace()).unwrap_or(buf.len());
        let end = buf[start..]
            .iter()
            .position(|b| !b.is_ascii_digit())
            .map(|i| start + i)
            .unwrap_or(buf.len());
        let digits = &buf[start..end];
        if digits.is_empty() {
            return Err(ErrorKind::InvalidInput);
        }

        let value: u8 = digits
            .iter()
            .try_fold(0u16, |acc, b| {
                let d = u16::from(b.wrapping_sub(b'0'));
                if d > 9 {
                    return None;
                }
                acc.checked_mul(10)?.checked_add(d)
            })
            .and_then(|v| (v <= 255).then_some(v as u8))
            .ok_or(ErrorKind::InvalidInput)?;

        self.ops.write().set_brightness(value)?;
        Ok(buf.len())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use blueos_test_macro::test;

    struct MockBacklight {
        brightness: u8,
    }

    impl BacklightOps for MockBacklight {
        fn set_brightness(&mut self, value: u8) -> Result<(), ErrorKind> {
            self.brightness = value;
            Ok(())
        }

        fn brightness(&self) -> Result<u8, ErrorKind> {
            Ok(self.brightness)
        }
    }

    fn mock_device() -> BacklightDevice<MockBacklight> {
        BacklightDevice {
            name: String::from("backlight"),
            id: DeviceId::new(240, 2),
            ops: Arc::new(RwLock::new(MockBacklight { brightness: 0xFF })),
        }
    }

    #[test]
    fn test_read_returns_decimal_string() {
        let device = mock_device();
        let mut buf = [0u8; 8];
        let len = device.read(0, &mut buf, true).unwrap();
        assert_eq!(&buf[..len], b"255\n");
    }

    #[test]
    fn test_write_parses_decimal() {
        let device = mock_device();
        let buf = b"128\n";
        device.write(0, buf, true).unwrap();
        let mut read_buf = [0u8; 8];
        let len = device.read(0, &mut read_buf, true).unwrap();
        assert_eq!(&read_buf[..len], b"128\n");
    }

    #[test]
    fn test_write_rejects_overflow() {
        let device = mock_device();
        let buf = b"256";
        assert_eq!(
            device.write(0, buf, true).unwrap_err(),
            ErrorKind::InvalidInput
        );
    }

    #[test]
    fn test_write_rejects_non_numeric() {
        let device = mock_device();
        let buf = b"abc";
        assert_eq!(
            device.write(0, buf, true).unwrap_err(),
            ErrorKind::InvalidInput
        );
    }
}