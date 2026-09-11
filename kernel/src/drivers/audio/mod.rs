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

pub(crate) mod axp2101;
pub(crate) mod es8311;

use core::sync::atomic::{AtomicU8, Ordering};

// 0=not attempted, 1=init failed, 2=init ok/verify failed,
// 3=init and verify succeeded, 4=I2C bus unavailable.
static ES8311_STATUS: AtomicU8 = AtomicU8::new(0);
// 0=not attempted, 1=configuration failed, 2=enabled and verified.
static SPEAKER_POWER_STATUS: AtomicU8 = AtomicU8::new(0);

pub(crate) fn set_es8311_status(status: u8) {
    ES8311_STATUS.store(status, Ordering::Relaxed);
}

pub(crate) fn es8311_status() -> u8 {
    ES8311_STATUS.load(Ordering::Relaxed)
}

pub(crate) fn set_speaker_power_status(status: u8) {
    SPEAKER_POWER_STATUS.store(status, Ordering::Relaxed);
}

pub(crate) fn speaker_power_status() -> u8 {
    SPEAKER_POWER_STATUS.load(Ordering::Relaxed)
}
