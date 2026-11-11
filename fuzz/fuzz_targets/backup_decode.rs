//! `BackupFile::decode`, `BackupContents::decode` and backup `unpad` must never panic, and accepted
//! files must be canonical. No key derivation runs here: decoding is checked alone.
#![no_main]

use libfuzzer_sys::fuzz_target;
use mem_proto::backup::{BackupContents, BackupFile, unpad};

fuzz_target!(|data: &[u8]| {
    if let Ok(file) = BackupFile::decode(data) {
        assert_eq!(file.encode(), data, "accepted a non-canonical backup file");
    }
    if let Ok(contents) = BackupContents::decode(data) {
        assert_eq!(contents.encode(), data, "non-canonical contents");
    }
    let _ = unpad(data);
});
