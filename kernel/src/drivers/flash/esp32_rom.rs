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

//! IRAM-resident wrappers for the ESP32-C6 mask-ROM Flash API.

use crate::arch::{disable_local_irq_save, enable_local_irq_restore};

pub const ESP_ROM_SPIFLASH_RESULT_OK: i32 = 0;

unsafe extern "C" {
    fn esp_rom_spiflash_read(src_addr: u32, data: *const u32, len: u32) -> i32;
    fn esp_rom_spiflash_write(dest_addr: u32, data: *const u32, len: u32) -> i32;
    fn esp_rom_spiflash_erase_sector(sector_number: u32) -> i32;
    fn esp_rom_spiflash_unlock() -> i32;
    fn spi_flash_get_chip_size() -> u32;
    fn Cache_Suspend_ICache() -> u32;
    fn Cache_Resume_ICache(state: u32);
}

#[inline(always)]
fn with_flash_op<R>(body: impl FnOnce() -> R) -> R {
    let irq_state = disable_local_irq_save();
    let cache_state = unsafe { Cache_Suspend_ICache() };
    let result = body();
    unsafe { Cache_Resume_ICache(cache_state) };
    enable_local_irq_restore(irq_state);
    result
}

#[link_section = ".rwtext"]
#[inline(never)]
pub(crate) unsafe fn rom_read(src_addr: u32, data: *const u32, len: u32) -> i32 {
    with_flash_op(|| unsafe { esp_rom_spiflash_read(src_addr, data, len) })
}

#[link_section = ".rwtext"]
#[inline(never)]
pub(crate) unsafe fn rom_write(dest_addr: u32, data: *const u32, len: u32) -> i32 {
    with_flash_op(|| unsafe { esp_rom_spiflash_write(dest_addr, data, len) })
}

#[link_section = ".rwtext"]
#[inline(never)]
pub(crate) unsafe fn rom_erase_sector(sector_index: u32) -> i32 {
    with_flash_op(|| unsafe { esp_rom_spiflash_erase_sector(sector_index) })
}

#[link_section = ".rwtext"]
#[inline(never)]
pub(crate) unsafe fn rom_unlock() -> i32 {
    with_flash_op(|| unsafe { esp_rom_spiflash_unlock() })
}

pub(crate) unsafe fn rom_chip_size() -> u32 {
    unsafe { spi_flash_get_chip_size() }
}
