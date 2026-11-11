package dev.meshm.mem

import android.app.Application

/**
 * Loads MEM's Rust library before any screen, push wake-up or service runs, so its JNI_OnLoad can
 * find [KeyGuard] with the app's class loader. Dart later opens the same library.
 */
class MemApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        try {
            System.loadLibrary("mem_ffi")
        } catch (_: UnsatisfiedLinkError) {
            // Without it the state key stays in its private file.
        }
    }
}
