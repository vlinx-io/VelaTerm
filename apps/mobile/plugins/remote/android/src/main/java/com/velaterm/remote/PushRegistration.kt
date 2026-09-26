package com.velaterm.remote

import android.app.NotificationChannel
import android.app.NotificationManager
import android.content.Context
import android.os.Build
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import androidx.core.app.NotificationManagerCompat
import org.json.JSONObject
import org.json.JSONArray
import java.net.URL
import java.security.KeyStore
import java.security.SecureRandom
import java.util.UUID
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec
import javax.net.ssl.HttpsURLConnection

/** A process-local singleton; the provider service reports credentials to the main-process receiver. */
internal class PushRegistration private constructor(private val context: Context) {
    companion object {
        @Volatile private var instance: PushRegistration? = null
        fun get(context: Context): PushRegistration = instance ?: synchronized(this) {
            instance ?: PushRegistration(context.applicationContext).also { instance = it }
        }
    }
    private val prefs = context.getSharedPreferences("vela-push", Context.MODE_PRIVATE)
    @Volatile private var lastError: String? = null
    @Volatile private var tokenWaiter: CountDownLatch? = null
    private val registrationWorker = java.util.concurrent.Executors.newSingleThreadExecutor()
    private val lock = Any()
    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey("vela-push", null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder("vela-push", KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT)
                .setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        }.generateKey()
    }
    private fun read(): JSONObject = synchronized(lock) {
        val encoded = prefs.getString("vault", null) ?: return@synchronized JSONObject()
        val bytes = Base64.decode(encoded, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes.copyOfRange(0, 12)))
        JSONObject(String(cipher.doFinal(bytes.copyOfRange(12, bytes.size)), Charsets.UTF_8))
    }
    private fun update(change: (JSONObject) -> Unit) = synchronized(lock) {
        val value = read(); change(value)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding"); cipher.init(Cipher.ENCRYPT_MODE, key())
        val encoded = Base64.encodeToString(cipher.iv + cipher.doFinal(value.toString().toByteArray()), Base64.NO_WRAP)
        check(prefs.edit().putString("vault", encoded).commit()) { "PUSH_STORAGE_FAILED" }
    }
    private fun owner(): String = synchronized(lock) {
        read().optString("owner").takeIf { it.isNotBlank() } ?: run {
            val bytes = ByteArray(32).also { SecureRandom().nextBytes(it) }
            val owner = Base64.encodeToString(bytes, Base64.URL_SAFE or Base64.NO_PADDING or Base64.NO_WRAP)
            update { it.put("owner", owner) }; owner
        }
    }
    fun configured(): Boolean {
        if (!PushProvider.AVAILABLE) return false
        val info = context.packageManager.getApplicationInfo(context.packageName, android.content.pm.PackageManager.GET_META_DATA)
        val metadata = info.metaData ?: return false
        if (metadata.getString("GETUI_APPID").isNullOrBlank()) return false
        val required = when (Build.MANUFACTURER.lowercase(java.util.Locale.ROOT)) {
            "huawei" -> listOf("com.huawei.hms.client.appid")
            "honor" -> listOf("com.hihonor.push.app_id")
            "xiaomi", "redmi" -> listOf("MIPUSH_APPID", "MIPUSH_APPKEY")
            "oppo", "oneplus", "realme" -> listOf("OPPOPUSH_APPKEY", "OPPOPUSH_APPSECRET")
            "vivo", "iqoo" -> listOf("api_key", "app_id")
            "meizu" -> listOf("MEIZUPUSH_APPID", "MEIZUPUSH_APPKEY")
            else -> emptyList()
        }
        if (required.any { metadata.get(it)?.toString().isNullOrBlank() }) return false
        if (Build.MANUFACTURER.equals("huawei", true) && context.assets.list("")?.contains("agconnect-services.json") != true) return false
        return true
    }
    private fun request(path: String, method: String = "POST", body: JSONObject? = null, account: String? = null): JSONObject {
        val connection = URL("https://velaterm.com/api/mobile-push$path").openConnection() as HttpsURLConnection
        try {
            connection.instanceFollowRedirects = false; connection.connectTimeout = 10000; connection.readTimeout = 20000
            connection.requestMethod = method; connection.setRequestProperty("Authorization", "Bearer ${owner()}")
            account?.let { connection.setRequestProperty("X-Vela-Account-Token", it) }
            if (body != null) {
                connection.doOutput = true; connection.setRequestProperty("Content-Type", "application/json")
                connection.outputStream.use { it.write(body.toString().toByteArray(Charsets.UTF_8)) }
            }
            check(connection.responseCode in 200..299) { "PUSH_RELAY_UNAVAILABLE" }
            val bytes = connection.inputStream.use { input ->
                val result = java.io.ByteArrayOutputStream(); val buffer = ByteArray(4096)
                while (result.size() < 16385) {
                    val count = input.read(buffer, 0, minOf(buffer.size, 16385 - result.size()))
                    if (count < 0) break
                    result.write(buffer, 0, count)
                }
                result.toByteArray()
            }
            check(bytes.size <= 16384) { "PUSH_RELAY_UNAVAILABLE" }
            return if (bytes.isEmpty()) JSONObject() else JSONObject(String(bytes, Charsets.UTF_8))
        } catch (_: Exception) { throw IllegalStateException("PUSH_RELAY_UNAVAILABLE") }
        finally { connection.disconnect() }
    }
    fun status(): JSONObject {
        val saved = read(); val allowed = NotificationManagerCompat.from(context).areNotificationsEnabled()
        val connections = JSONArray(); val subscriptions = saved.optJSONObject("subscriptions") ?: JSONObject()
        for (id in subscriptions.keys()) if (subscriptions.optJSONObject(id)?.optBoolean("bound") == true) connections.put(id)
        return JSONObject().put("configured", configured()).put("permission", if (allowed) "granted" else "denied")
            .put("enabled", configured() && saved.optBoolean("enabled") && saved.has("registeredToken") && allowed)
            .put("connections", connections).apply { lastError?.let { put("error", it) } }
    }
    // Called only after the app has shown its provider disclosure and obtained notification consent.
    fun initialize() {
        check(configured()) { "PUSH_NOT_CONFIGURED" }
        if (Build.VERSION.SDK_INT >= 26) context.getSystemService(NotificationManager::class.java)
            .createNotificationChannel(NotificationChannel("task-updates", "VelaTerm", NotificationManager.IMPORTANCE_HIGH))
        PushProvider.initialize(context)
    }
    fun enable() {
        check(NotificationManagerCompat.from(context).areNotificationsEnabled()) { "PUSH_DENIED" }
        update { it.put("enabled", true) }
        tokenWaiter = CountDownLatch(1)
        try {
            initialize()
            val existing = PushProvider.clientId(context)
            if (!existing.isNullOrBlank()) token(existing)
            check(tokenWaiter?.await(20, TimeUnit.SECONDS) == true) { "PUSH_REGISTRATION_FAILED" }
            register(read().getString("token")); lastError = null
        } catch (error: Exception) { lastError = error.message?.takeIf { it.startsWith("PUSH_") } ?: "PUSH_REGISTRATION_FAILED"; throw IllegalStateException(lastError) }
        finally { tokenWaiter = null }
    }
    fun token(value: String) {
        if (!value.matches(Regex("[A-Za-z0-9_-]{16,128}"))) return
        try { update { it.put("token", value) } }
        catch (_: Exception) { lastError = "PUSH_REGISTRATION_FAILED"; tokenWaiter?.countDown(); return }
        tokenWaiter?.countDown()
        registrationWorker.execute {
            try {
                val saved = read()
                if (saved.optBoolean("enabled") && saved.optString("registeredToken") != value) register(value)
            } catch (_: Exception) { lastError = "PUSH_RELAY_UNAVAILABLE" }
        }
    }
    @Synchronized private fun register(value: String) {
        if (!read().optBoolean("enabled")) return
        request("/installations", body = JSONObject().put("platform", "getui").put("environment", "production")
            .put("token", value).put("locale", context.resources.configuration.locales[0].toLanguageTag()))
        update { it.put("registeredToken", value).put("registeredAt", System.currentTimeMillis()) }
    }
    fun resume() { registrationWorker.execute {
        try {
            val saved = read()
            if (!saved.optBoolean("enabled")) return@execute
            if (!NotificationManagerCompat.from(context).areNotificationsEnabled()) { disable(); return@execute }
            initialize()
            val token = PushProvider.clientId(context)
            if (!token.isNullOrBlank() && (saved.optString("registeredToken") != token || System.currentTimeMillis() - saved.optLong("registeredAt") > 86400000)) register(token)
        } catch (_: Exception) { lastError = "PUSH_RELAY_UNAVAILABLE" }
    } }
    fun subscription(connection: String, account: String? = null): JSONObject {
        val saved = read(); check(saved.optBoolean("enabled") && saved.has("registeredToken")) { "PUSH_DISABLED" }
        val result = request("/subscriptions", body = JSONObject().put("connectionId", connection), account = account)
        UUID.fromString(result.getString("subscriptionId"))
        check(result.getString("publisherToken").matches(Regex("[A-Za-z0-9_-]{43}"))) { "PUSH_RELAY_UNAVAILABLE" }
        update { store ->
            val subscriptions = store.optJSONObject("subscriptions") ?: JSONObject()
            val previous = subscriptions.optJSONObject(connection)
            val bound = previous?.optString("subscriptionId") == result.getString("subscriptionId") && previous.optBoolean("bound")
            subscriptions.put(connection, JSONObject(result.toString()).put("bound", bound)); store.put("subscriptions", subscriptions)
        }
        return result
    }
    fun bound(connection: String, active: Boolean) {
        // Retain an acknowledged host subscription across transient reconnect failures.
        update { it.optJSONObject("subscriptions")?.optJSONObject(connection)?.let { entry -> entry.put("bound", active || entry.optBoolean("bound")) } }
        lastError = if (active) null else "PUSH_HOST_UNAVAILABLE"
    }
    fun isBound(connection: String): Boolean { val saved = read(); return configured() && saved.optBoolean("enabled") && saved.optJSONObject("subscriptions")?.optJSONObject(connection)?.optBoolean("bound") == true }
    fun revoke(connection: String) {
        val id = read().optJSONObject("subscriptions")?.optJSONObject(connection)?.optString("subscriptionId") ?: return
        request("/subscriptions/$id", "DELETE")
        update { it.optJSONObject("subscriptions")?.remove(connection) }
    }
    fun revokeAccount() {
        val subscriptions = read().optJSONObject("subscriptions") ?: return
        for (id in subscriptions.keys().asSequence().toList()) if (id.startsWith("account_")) revoke(id)
    }
    @Synchronized fun disable() {
        if (read().has("registeredToken")) request("/installation", "DELETE")
        update { it.put("enabled", false).remove("registeredToken"); it.put("subscriptions", JSONObject()) }
        PushProvider.turnOff(context); lastError = null
    }
    fun test(connection: String) {
        val id = read().optJSONObject("subscriptions")?.optJSONObject(connection)?.optString("subscriptionId") ?: error("PUSH_HOST_UNAVAILABLE")
        request("/subscriptions/$id/test", body = JSONObject())
    }
}
