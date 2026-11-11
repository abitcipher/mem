//! The ggwave C API (vendor/ggwave/include/ggwave/ggwave.h), only the calls MEM uses.

use std::ffi::{c_int, c_void};

/// `ggwave_Parameters`.
#[repr(C)]
#[derive(Clone, Copy)]
pub(crate) struct Parameters {
    pub(crate) payload_length: c_int,
    pub(crate) sample_rate_inp: f32,
    pub(crate) sample_rate_out: f32,
    pub(crate) sample_rate: f32,
    pub(crate) samples_per_frame: c_int,
    pub(crate) sound_marker_threshold: f32,
    pub(crate) sample_format_inp: c_int,
    pub(crate) sample_format_out: c_int,
    pub(crate) operating_mode: c_int,
}

/// `GGWAVE_SAMPLE_FORMAT_I16`.
pub(crate) const SAMPLE_FORMAT_I16: c_int = 4;
/// `GGWAVE_OPERATING_MODE_RX` / `_TX`.
pub(crate) const MODE_RX: c_int = 1 << 1;
pub(crate) const MODE_TX: c_int = 1 << 2;
/// `GGWAVE_PROTOCOL_AUDIBLE_FAST` / `GGWAVE_PROTOCOL_ULTRASOUND_FAST`. ("Fastest" decoded on a
/// laptop's own speaker and microphone but not across a room: echo smears its short tones.)
pub(crate) const PROTOCOL_AUDIBLE_FAST: c_int = 1;
pub(crate) const PROTOCOL_ULTRASOUND_FAST: c_int = 4;
/// `GGWAVE_PROTOCOL_COUNT` without `GGWAVE_CONFIG_FEW_PROTOCOLS`.
pub(crate) const PROTOCOL_COUNT: c_int = 22;

unsafe extern "C" {
    pub(crate) fn ggwave_setLogFile(fptr: *mut c_void);
    pub(crate) fn ggwave_getDefaultParameters() -> Parameters;
    pub(crate) fn ggwave_init(parameters: Parameters) -> c_int;
    pub(crate) fn ggwave_free(instance: c_int);
    pub(crate) fn ggwave_encode(
        instance: c_int,
        payload: *const c_void,
        payload_size: c_int,
        protocol: c_int,
        volume: c_int,
        waveform: *mut c_void,
        query: c_int,
    ) -> c_int;
    pub(crate) fn ggwave_ndecode(
        instance: c_int,
        waveform: *const c_void,
        waveform_size: c_int,
        payload: *mut c_void,
        payload_size: c_int,
    ) -> c_int;
    pub(crate) fn ggwave_rxToggleProtocol(protocol: c_int, state: c_int);
    /// MEM's shim (`shim/mem_ggwave.cpp`): 1 while the instance is receiving a transmission.
    pub(crate) fn mem_ggwave_rx_receiving(instance: c_int) -> c_int;
}
