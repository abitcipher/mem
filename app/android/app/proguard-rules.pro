# Called from Rust through JNI by name: keep the class and its static methods.
-keep class dev.meshm.mem.KeyGuard { public static byte[] wrap(byte[]); public static byte[] unwrap(byte[]); }
