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

//! ESP32-C6 internal Flash access through the mask-ROM API.

use super::esp32_rom;
use crate::{scheduler, sync::Mutex, time::Tick};
use core::sync::atomic::{AtomicU32, Ordering};

pub const ESP_FLASH_SECTOR_SIZE: usize = 4096;
const ESP_FLASH_WORD_SIZE: usize = 4;
const ROM_PAGE_SIZE: usize = 256;
const READ_CHUNK_SIZE: usize = 1024;

crate::static_arc! {
    INTERNAL_FLASH_LOCK(Mutex, Mutex::new()),
}

static INTERNAL_FLASH_CAPACITY: AtomicU32 = AtomicU32::new(0);

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum EspFlashError {
    OutOfBounds,
    UnalignedErase,
    Busy,
    RomError(i32),
}

struct FlashGuard {
    locked: bool,
}

impl FlashGuard {
    fn acquire() -> Result<Self, EspFlashError> {
        let locked = scheduler::is_schedule_ready();
        if locked && !INTERNAL_FLASH_LOCK.pend_for(Tick::MAX) {
            return Err(EspFlashError::Busy);
        }
        Ok(Self { locked })
    }
}

impl Drop for FlashGuard {
    fn drop(&mut self) {
        if self.locked {
            INTERNAL_FLASH_LOCK.post();
        }
    }
}

pub fn init_internal_flash() -> Result<(), EspFlashError> {
    if !INTERNAL_FLASH_LOCK.init() {
        return Err(EspFlashError::Busy);
    }
    Ok(())
}

pub fn with_internal_flash<R>(
    operation: impl FnOnce(&mut Esp32c6InternalFlash) -> Result<R, EspFlashError>,
) -> Result<R, EspFlashError> {
    let _guard = FlashGuard::acquire()?;
    let mut capacity = INTERNAL_FLASH_CAPACITY.load(Ordering::Acquire);
    if capacity == 0 {
        let result = unsafe { esp32_rom::rom_unlock() };
        rom_result("unlock", 0, 0, result)?;
        capacity = unsafe { esp32_rom::rom_chip_size() };
        if capacity == 0 {
            crate::kprintln!("[FLASH] chip-size probe returned zero");
            return Err(EspFlashError::OutOfBounds);
        }
        crate::kprintln!("[FLASH] detected capacity={} bytes", capacity);
        INTERNAL_FLASH_CAPACITY.store(capacity, Ordering::Release);
    }
    operation(&mut Esp32c6InternalFlash { capacity })
}

fn rom_result(operation: &str, address: u32, len: u32, result: i32) -> Result<(), EspFlashError> {
    if result == esp32_rom::ESP_ROM_SPIFLASH_RESULT_OK {
        Ok(())
    } else {
        crate::kprintln!(
            "[FLASH] ROM {} failed: address=0x{:08x} len={} result={}",
            operation,
            address,
            len,
            result
        );
        Err(EspFlashError::RomError(result))
    }
}

pub struct Esp32c6InternalFlash {
    capacity: u32,
}

impl Esp32c6InternalFlash {
    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    fn check_bounds(&self, offset: u32, len: usize) -> Result<(), EspFlashError> {
        let len = u32::try_from(len).map_err(|_| EspFlashError::OutOfBounds)?;
        let end = offset.checked_add(len).ok_or(EspFlashError::OutOfBounds)?;
        if end > self.capacity {
            Err(EspFlashError::OutOfBounds)
        } else {
            Ok(())
        }
    }

    pub fn read(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), EspFlashError> {
        self.check_bounds(offset, buf.len())?;
        let mut done = 0usize;
        while done < buf.len() {
            let absolute = offset + done as u32;
            let aligned = absolute & !(ESP_FLASH_WORD_SIZE as u32 - 1);
            let skip = (absolute - aligned) as usize;
            let remaining = buf.len() - done;
            let useful = core::cmp::min(remaining, READ_CHUNK_SIZE - skip);
            let rom_len = (skip + useful + ESP_FLASH_WORD_SIZE - 1) & !(ESP_FLASH_WORD_SIZE - 1);
            let mut scratch = AlignedBuffer::<READ_CHUNK_SIZE>::zeroed();
            let result = unsafe {
                esp32_rom::rom_read(aligned, scratch.0.as_ptr() as *const u32, rom_len as u32)
            };
            rom_result("read", aligned, rom_len as u32, result)?;
            buf[done..done + useful].copy_from_slice(&scratch.0[skip..skip + useful]);
            done += useful;
        }
        Ok(())
    }

    pub fn erase_region(&mut self, offset: u32, len: u32) -> Result<(), EspFlashError> {
        if offset % ESP_FLASH_SECTOR_SIZE as u32 != 0
            || len == 0
            || len % ESP_FLASH_SECTOR_SIZE as u32 != 0
        {
            return Err(EspFlashError::UnalignedErase);
        }
        self.check_bounds(offset, len as usize)?;
        let first = offset / ESP_FLASH_SECTOR_SIZE as u32;
        let count = len / ESP_FLASH_SECTOR_SIZE as u32;
        for sector in first..first + count {
            let result = unsafe { esp32_rom::rom_erase_sector(sector) };
            rom_result(
                "erase",
                sector * ESP_FLASH_SECTOR_SIZE as u32,
                ESP_FLASH_SECTOR_SIZE as u32,
                result,
            )?;
        }
        Ok(())
    }

    pub fn write(&mut self, offset: u32, data: &[u8]) -> Result<(), EspFlashError> {
        self.check_bounds(offset, data.len())?;
        if data.is_empty() {
            return Ok(());
        }

        let data_end = offset
            .checked_add(u32::try_from(data.len()).map_err(|_| EspFlashError::OutOfBounds)?)
            .ok_or(EspFlashError::OutOfBounds)?;
        let mut current = offset & !(ESP_FLASH_WORD_SIZE as u32 - 1);
        let aligned_end = data_end
            .checked_add(ESP_FLASH_WORD_SIZE as u32 - 1)
            .map(|end| end & !(ESP_FLASH_WORD_SIZE as u32 - 1))
            .ok_or(EspFlashError::OutOfBounds)?;
        let mut page = AlignedBuffer::<ROM_PAGE_SIZE>::erased();

        while current < aligned_end {
            let page_left = ROM_PAGE_SIZE - current as usize % ROM_PAGE_SIZE;
            let write_len = core::cmp::min(page_left, (aligned_end - current) as usize);
            page.0[..write_len].fill(0xff);
            let copy_start = core::cmp::max(current, offset);
            let copy_end = core::cmp::min(current + write_len as u32, data_end);
            if copy_start < copy_end {
                let src = (copy_start - offset) as usize;
                let dst = (copy_start - current) as usize;
                let len = (copy_end - copy_start) as usize;
                page.0[dst..dst + len].copy_from_slice(&data[src..src + len]);
            }
            let result = unsafe {
                esp32_rom::rom_write(current, page.0.as_ptr() as *const u32, write_len as u32)
            };
            rom_result("write", current, write_len as u32, result)?;
            current += write_len as u32;
        }
        Ok(())
    }
}

#[repr(align(4))]
struct AlignedBuffer<const N: usize>([u8; N]);

impl<const N: usize> AlignedBuffer<N> {
    const fn zeroed() -> Self {
        Self([0; N])
    }

    const fn erased() -> Self {
        Self([0xff; N])
    }
}
