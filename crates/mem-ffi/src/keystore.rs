//! Android Keystore as the state key's [`KeyGuard`].
//!
//! The Keystore is reachable only from the JVM, so the app's `MemApplication` loads this library
//! with `System.loadLibrary` before anything else runs. `JNI_OnLoad` then keeps the Java VM and
//! the app's `dev.meshm.mem.KeyGuard` class (only the app's class loader can find it). Rust calls
//! its static `wrap` and `unwrap` directly: the key never passes through Dart.

#![allow(
    unsafe_code,
    reason = "JNI_OnLoad must be an exported, unmangled symbol"
)]

use std::ffi::c_void;
use std::sync::{Arc, OnceLock};

use jni::objects::{GlobalRef, JByteArray, JClass, JObject, JValue};
use jni::sys::{JNI_VERSION_1_6, jint};
use jni::{JNIEnv, JavaVM};
use mem_runtime::{KeyGuard, KeyGuardError};
use zeroize::Zeroizing;

static JVM: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

/// Called by Android when `MemApplication` loads the library.
#[unsafe(no_mangle)]
pub extern "system" fn JNI_OnLoad(vm: JavaVM, _reserved: *mut c_void) -> jint {
    if let Ok(mut env) = vm.get_env() {
        match env.find_class("dev/meshm/mem/KeyGuard") {
            Ok(class) => {
                if let Ok(class) = env.new_global_ref(class) {
                    let _ = JVM.set((vm, class));
                }
            }
            Err(_) => {
                let _ = env.exception_clear();
            }
        }
    }
    JNI_VERSION_1_6
}

/// The Keystore guard, if the app loaded the library from Kotlin (it always does).
pub(crate) fn guard() -> Option<Arc<dyn KeyGuard>> {
    JVM.get()
        .map(|_| Arc::new(AndroidKeystore) as Arc<dyn KeyGuard>)
}

#[derive(Debug)]
struct AndroidKeystore;

impl AndroidKeystore {
    /// Calls `KeyGuard.<method>(byte[]): byte[]` and returns the result, zeroing the Java input
    /// array when it held the key.
    fn call(
        method: &str,
        input: &[u8],
        clear_input: bool,
    ) -> Result<Zeroizing<Vec<u8>>, KeyGuardError> {
        let (vm, class) = JVM.get().ok_or(KeyGuardError)?;
        let mut env = vm.attach_current_thread().map_err(|_| KeyGuardError)?;
        let result = Self::call_in(&mut env, class, method, input, clear_input);
        if env.exception_check().unwrap_or(false) {
            let _ = env.exception_clear();
        }
        result
    }

    fn call_in(
        env: &mut JNIEnv<'_>,
        class: &GlobalRef,
        method: &str,
        input: &[u8],
        clear_input: bool,
    ) -> Result<Zeroizing<Vec<u8>>, KeyGuardError> {
        let class: &JClass<'_> = class.as_obj().into();
        let arg = env
            .byte_array_from_slice(input)
            .map_err(|_| KeyGuardError)?;
        let out = env.call_static_method(class, method, "([B)[B", &[JValue::Object(&arg)]);
        if clear_input {
            let zeros = vec![0i8; input.len()];
            let _ = env.set_byte_array_region(&arg, 0, &zeros);
        }
        let out: JObject<'_> = out
            .and_then(jni::objects::JValueGen::l)
            .map_err(|_| KeyGuardError)?;
        if out.is_null() {
            return Err(KeyGuardError);
        }
        let out = JByteArray::from(out);
        let bytes = Zeroizing::new(env.convert_byte_array(&out).map_err(|_| KeyGuardError)?);
        if !clear_input {
            // The output is the key: clear the Java copy too.
            let zeros = vec![0i8; bytes.len()];
            let _ = env.set_byte_array_region(&out, 0, &zeros);
        }
        Ok(bytes)
    }
}

impl KeyGuard for AndroidKeystore {
    fn wrap(&self, key: &[u8; 32]) -> Result<Vec<u8>, KeyGuardError> {
        Self::call("wrap", key, true).map(|v| v.to_vec())
    }

    fn unwrap(&self, wrapped: &[u8]) -> Result<Zeroizing<[u8; 32]>, KeyGuardError> {
        let key = Self::call("unwrap", wrapped, false)?;
        let key: [u8; 32] = key.as_slice().try_into().map_err(|_| KeyGuardError)?;
        Ok(Zeroizing::new(key))
    }
}
