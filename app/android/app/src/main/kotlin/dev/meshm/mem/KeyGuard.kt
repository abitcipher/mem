package dev.meshm.mem

import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import java.security.KeyStore
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

/**
 * Wraps MEM's 32-byte state key with an AES-256-GCM key that lives in Android Keystore and never
 * leaves it. Called only from Rust through JNI (`mem_ffi::keystore`); the state key
 * never passes through Dart.
 *
 * Wrapped form: `version(1) || iv(12) || ciphertext(32) || tag(16)`, with the associated data
 * "MEM/v1/state-key-wrap".
 */
object KeyGuard {
    private const val ALIAS = "mem_state_key_wrap"
    private const val VERSION: Byte = 1
    private const val IV_LEN = 12
    private const val TAG_BITS = 128
    private val AAD = "MEM/v1/state-key-wrap".toByteArray(Charsets.US_ASCII)

    @JvmStatic
    fun wrap(key: ByteArray): ByteArray {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.ENCRYPT_MODE, wrappingKey())
        cipher.updateAAD(AAD)
        val sealed = cipher.doFinal(key)
        val iv = cipher.iv
        require(iv.size == IV_LEN)
        return byteArrayOf(VERSION) + iv + sealed
    }

    @JvmStatic
    fun unwrap(wrapped: ByteArray): ByteArray {
        require(wrapped.size > 1 + IV_LEN && wrapped[0] == VERSION)
        val key = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
            .getKey(ALIAS, null) as? SecretKey ?: throw IllegalStateException("no wrapping key")
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(
            Cipher.DECRYPT_MODE,
            key,
            GCMParameterSpec(TAG_BITS, wrapped, 1, IV_LEN),
        )
        cipher.updateAAD(AAD)
        return cipher.doFinal(wrapped, 1 + IV_LEN, wrapped.size - 1 - IV_LEN)
    }

    /** The Keystore key, created on first use: no user authentication, so background work runs. */
    private fun wrappingKey(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey(ALIAS, null) as? SecretKey)?.let { return it }
        val generator = KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore")
        generator.init(
            KeyGenParameterSpec.Builder(
                ALIAS,
                KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT,
            )
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM)
                .setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE)
                .setKeySize(256)
                .build(),
        )
        return generator.generateKey()
    }
}
