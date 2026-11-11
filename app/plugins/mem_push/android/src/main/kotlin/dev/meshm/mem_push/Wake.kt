package dev.meshm.mem_push

import android.content.Context
import android.os.Handler
import android.os.Looper
import io.flutter.FlutterInjector
import io.flutter.embedding.engine.FlutterEngine
import io.flutter.embedding.engine.FlutterEngineCache
import io.flutter.embedding.engine.dart.DartExecutor
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit

/**
 * Shared state between the push receiver, the wake worker and the Flutter plugin.
 *
 * A wake-up carries no data; it only means "fetch from your hub now". The worker makes sure the
 * app's Dart side runs (the same cached engine the activity uses, so the node is opened once),
 * asks it to fetch, and waits until it says it is done or [MAX_WAKE_MS] passes.
 */
internal object Wake {
    /** The engine the app's MainActivity caches (MainActivity.ENGINE). */
    const val ENGINE = "mem"
    const val MAX_WAKE_MS = 25_000L
    const val PREFS = "mem-push"
    const val KEY_ENDPOINT = "endpoint"

    /** The plugin instance attached to the running engine, if any. */
    @Volatile
    var plugin: MemPushPlugin? = null

    /** A wake-up the Dart side has not picked up yet (it was not running). */
    @Volatile
    var pending = false

    @Volatile
    private var done: CountDownLatch? = null

    fun saveEndpoint(context: Context, url: String?) {
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).edit().apply {
            if (url == null) remove(KEY_ENDPOINT) else putString(KEY_ENDPOINT, url)
        }.apply()
    }

    fun endpoint(context: Context): String? =
        context.getSharedPreferences(PREFS, Context.MODE_PRIVATE).getString(KEY_ENDPOINT, null)

    /** Runs one wake-up; blocks the calling (worker) thread until done or timed out. */
    fun run(context: Context) {
        val latch = CountDownLatch(1)
        done = latch
        Handler(Looper.getMainLooper()).post {
            val p = plugin
            if (p != null) {
                p.wake()
            } else {
                // No Dart running: start the app's engine headless. Its main() opens the node
                // (the hub connects by itself if on) and asks for the pending wake-up.
                pending = true
                if (FlutterEngineCache.getInstance().get(ENGINE) == null) {
                    val loader = FlutterInjector.instance().flutterLoader()
                    loader.startInitialization(context.applicationContext)
                    loader.ensureInitializationComplete(context.applicationContext, null)
                    val engine = FlutterEngine(context.applicationContext)
                    engine.dartExecutor.executeDartEntrypoint(DartExecutor.DartEntrypoint.createDefault())
                    FlutterEngineCache.getInstance().put(ENGINE, engine)
                }
            }
        }
        latch.await(MAX_WAKE_MS, TimeUnit.MILLISECONDS)
        done = null
    }

    /** The Dart side finished fetching. */
    fun finished() {
        pending = false
        done?.countDown()
    }
}
