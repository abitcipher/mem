//! The device objects arrive inside key updates and messages from contacts, and
//! as QR codes: their decoders must never panic, and anything they accept must be the canonical
//! encoding of what they return (signatures cover it).
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::device::{DeviceCert, DeviceList, DeviceRequest, DeviceSetup, OwnSync};

fuzz_target!(|data: &[u8]| {
    if let Ok(c) = DeviceCert::decode(data) {
        assert_eq!(c.encode(), data, "non-canonical {c:?}");
    }
    if let Ok(l) = DeviceList::decode(data) {
        assert_eq!(l.encode().ok().as_deref(), Some(data), "non-canonical {l:?}");
    }
    if let Ok(r) = DeviceRequest::decode(data) {
        assert_eq!(r.encode(), data, "non-canonical {r:?}");
    }
    if let Ok(s) = DeviceSetup::decode(data) {
        assert_eq!(s.encode().ok().as_deref(), Some(data), "non-canonical {s:?}");
    }
    if let Ok(o) = OwnSync::decode(data) {
        assert_eq!(o.encode().ok().as_deref(), Some(data), "non-canonical {o:?}");
    }
});
