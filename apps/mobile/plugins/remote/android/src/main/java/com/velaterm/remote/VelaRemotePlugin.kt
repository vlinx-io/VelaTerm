package com.velaterm.remote

import android.app.Dialog
import android.app.AlertDialog
import android.content.Context
import android.os.Handler
import android.os.Looper
import android.security.keystore.KeyGenParameterSpec
import android.security.keystore.KeyProperties
import android.util.Base64
import android.webkit.WebView
import android.webkit.WebViewClient
import android.webkit.WebResourceRequest
import android.webkit.WebResourceError
import android.widget.LinearLayout
import android.widget.Button
import android.widget.TextView
import androidx.webkit.WebViewCompat
import androidx.webkit.WebViewFeature
import com.getcapacitor.*
import com.getcapacitor.annotation.CapacitorPlugin
import net.schmizz.sshj.SSHClient
import net.schmizz.sshj.connection.channel.direct.Parameters
import net.schmizz.sshj.common.Buffer
import com.hierynomus.sshj.userauth.keyprovider.OpenSSHKeyV1KeyFile
import net.schmizz.sshj.userauth.password.PasswordUtils
import org.json.JSONObject
import org.json.JSONArray
import java.net.InetAddress
import java.net.ServerSocket
import java.net.URI
import java.nio.charset.StandardCharsets
import java.security.KeyStore
import java.security.MessageDigest
import java.security.SecureRandom
import java.util.UUID
import java.util.concurrent.Executors
import java.util.concurrent.CountDownLatch
import java.util.concurrent.TimeUnit
import javax.crypto.Cipher
import javax.crypto.KeyGenerator
import javax.crypto.SecretKey
import javax.crypto.spec.GCMParameterSpec

@CapacitorPlugin(name = "VelaRemote")
class VelaRemotePlugin : Plugin() {
    private val worker = Executors.newSingleThreadExecutor()
    private val cleanupWorker = Executors.newSingleThreadExecutor()
    private var reconnectPage: (() -> Unit)? = null
    private val main = Handler(Looper.getMainLooper())
    @Volatile private var client: SSHClient? = null
    @Volatile private var listener: ServerSocket? = null
    private val generation = java.util.concurrent.atomic.AtomicInteger(0)
    private var browser: Dialog? = null
    private var web: WebView? = null
    private var activeId: String? = null
    private var requestedAccountSession: String? = null
    private lateinit var notifications: TaskNotifications
    private var pageStatus: TextView? = null
    private var recovery: ConnectionRecoveryView? = null
    private var suspended=false
    private var pickingFile=false
    private var fileResult: android.webkit.ValueCallback<Array<android.net.Uri>>? = null
    private var chooser: androidx.activity.result.ActivityResultLauncher<android.content.Intent>? = null
    private var downloadChooser: androidx.activity.result.ActivityResultLauncher<android.content.Intent>? = null
    private val downloadPending=java.util.concurrent.atomic.AtomicBoolean(false)
    private var downloadBytes:ByteArray? = null
    private var scanCall: PluginCall? = null
    private var scanner: androidx.activity.result.ActivityResultLauncher<com.journeyapps.barcodescanner.ScanOptions>? = null
    private val texts by lazy { MobileText(context) }
    override fun load() {
        notifications = TaskNotifications(activity)
        consumeNotification(activity.intent)
        scanner=activity.activityResultRegistry.register("vela-remote-qr",com.journeyapps.barcodescanner.ScanContract()) { result ->
            val call=scanCall;scanCall=null
            if(call!=null) {
                if(result.originalIntent?.getBooleanExtra("MISSING_CAMERA_PERMISSION",false)==true) {
                    call.reject(texts.get("mobile.native.cameraPermissionDenied"), "CAMERA_PERMISSION_DENIED")
                } else if(result.contents==null) {
                    call.resolve(JSObject().put("cancelled",true))
                } else {
                    try {
                        val text=result.contents
                        if(text.toByteArray(Charsets.UTF_8).size>8192) fail(texts.get("mobile.native.qrTooLong"))
                        val uri=safeUrl(text.trim())
                        call.resolve(JSObject().put("url",uri.toASCIIString()).put("name",uri.host))
                    } catch(_:Exception) {
                        call.reject(texts.get("mobile.native.qrInvalid"), "INVALID_QR_URL")
                    }
                }
            }
        }

        downloadChooser=activity.activityResultRegistry.register("vela-remote-download",androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult()) { result ->
            val bytes=downloadBytes;downloadBytes=null;downloadPending.set(false)
            val target=result.data?.data
            if(result.resultCode==android.app.Activity.RESULT_OK && target!=null && bytes!=null) worker.execute {
                try {
                    context.contentResolver.openOutputStream(target)?.use{it.write(bytes)} ?: fail(texts.get("mobile.native.saveLocationFailed"))
                    main.post{pageStatus?.text=texts.get("mobile.native.fileSaved")}
                } catch(_:Exception){main.post{pageStatus?.text=texts.get("mobile.native.fileSaveFailed")}}
            }
        }
        chooser=activity.activityResultRegistry.register("vela-remote-files",androidx.activity.result.contract.ActivityResultContracts.StartActivityForResult()) { result ->
            fileResult?.onReceiveValue(android.webkit.WebChromeClient.FileChooserParams.parseResult(result.resultCode,result.data));fileResult=null
        }
    }
    @PluginMethod fun notifications(call: PluginCall) {
        val action = call.getString("action") ?: "status"
        val perform = {
            worker.execute {
                try {
                    val push = PushRegistration.get(context)
                    when (action) {
                        "enable" -> push.enable()
                        "disable" -> push.disable()
                        "test" -> push.test(call.getString("id") ?: "")
                        "status" -> Unit
                        else -> error("PUSH_DISABLED")
                    }
                    call.resolve(JSObject(push.status().toString()))
                } catch (error: Exception) {
                    val code = error.message?.takeIf { it.matches(Regex("PUSH_[A-Z_]+")) } ?: "PUSH_RELAY_UNAVAILABLE"
                    call.reject(code, code)
                }
            }
        }
        if (action == "enable") main.post {
            val text = MobileText(context)
            if (!PushRegistration.get(context).configured()) { call.reject("PUSH_NOT_CONFIGURED", "PUSH_NOT_CONFIGURED"); return@post }
            AlertDialog.Builder(activity).setTitle(text.get("mobile.pushTitle")).setMessage(text.get("mobile.pushDisclosure"))
                .setNegativeButton(text.get("common.cancel")) { _, _ -> call.reject("PUSH_DISABLED", "PUSH_DISABLED") }
                .setPositiveButton(text.get("mobile.pushEnable")) { _, _ -> notifications.permission(true) { permission ->
                    if (permission == "granted") perform() else call.reject("PUSH_DENIED", "PUSH_DENIED")
                } }.setOnCancelListener { call.reject("PUSH_DISABLED", "PUSH_DISABLED") }.show()
        } else perform()
    }
    @PluginMethod fun scanURL(call: PluginCall) {
        main.post {
            if(scanCall!=null) {call.reject(texts.get("mobile.native.scanBusy"), "SCAN_BUSY");return@post}
            try {
                val launcher=scanner ?: fail(texts.get("mobile.native.scannerNotReady"))
                scanCall=call;pickingFile=true
                launcher.launch(com.journeyapps.barcodescanner.ScanOptions()
                    .setDesiredBarcodeFormats(com.journeyapps.barcodescanner.ScanOptions.QR_CODE)
                    .setPrompt(texts.get("mobile.native.scanPrompt"))
                    .setBeepEnabled(false).setBarcodeImageEnabled(false).setOrientationLocked(false)
                    .addExtra("SHOW_MISSING_CAMERA_PERMISSION_DIALOG",false))
            } catch(_:Exception) {
                scanCall=null;pickingFile=false
                call.reject(texts.get("mobile.native.cameraUnavailable"), "SCAN_UNAVAILABLE")
            }
        }
    }
    private val prefs get() = context.getSharedPreferences("vela-remote", Context.MODE_PRIVATE)
    private fun fail(message: String): Nothing = throw IllegalArgumentException(message)
    private fun run(call: PluginCall, block: () -> JSObject) {
        worker.execute {
            try { call.resolve(block()) }
            catch (e: AccountFailure) {
                if(e.code=="ACCOUNT_LOGIN_EXPIRED") {
                    accountAttempt=null
                    try {writeStore(readStore().apply {remove("accountAttempt")})}
                    catch(storageError:Exception) {call.reject(storageError.message ?: texts.get("mobile.native.loginStateUpdateFailed"),"REMOTE_ERROR");return@execute}
                }
                call.reject(e.message ?: texts.get("mobile.native.loginFailed"),e.code)
            }
            catch (e: Exception) { call.reject(e.message ?: texts.get("mobile.native.connectionFailed"), "REMOTE_ERROR") }
        }
    }
    private fun key(): SecretKey {
        val store = KeyStore.getInstance("AndroidKeyStore").apply { load(null) }
        (store.getKey("vela-remote", null) as? SecretKey)?.let { return it }
        return KeyGenerator.getInstance(KeyProperties.KEY_ALGORITHM_AES, "AndroidKeyStore").apply {
            init(KeyGenParameterSpec.Builder("vela-remote", KeyProperties.PURPOSE_ENCRYPT or KeyProperties.PURPOSE_DECRYPT).setBlockModes(KeyProperties.BLOCK_MODE_GCM).setEncryptionPaddings(KeyProperties.ENCRYPTION_PADDING_NONE).build())
        }.generateKey()
    }
    private fun readStore(): JSONObject {
        val encoded = prefs.getString("vault", null) ?: return JSONObject().put("connections", JSONArray()).put("keys", JSONObject())
        val bytes = Base64.decode(encoded, Base64.NO_WRAP)
        val cipher = Cipher.getInstance("AES/GCM/NoPadding")
        cipher.init(Cipher.DECRYPT_MODE, key(), GCMParameterSpec(128, bytes.copyOfRange(0,12)))
        return JSONObject(String(cipher.doFinal(bytes.copyOfRange(12,bytes.size)), StandardCharsets.UTF_8))
    }
    private fun writeStore(store: JSONObject) {
        val cipher = Cipher.getInstance("AES/GCM/NoPadding"); cipher.init(Cipher.ENCRYPT_MODE,key())
        val value = Base64.encodeToString(cipher.iv + cipher.doFinal(store.toString().toByteArray()),Base64.NO_WRAP)
        if (!prefs.edit().putString("vault",value).commit()) fail(texts.get("mobile.native.secureStorageWriteFailed"))
    }
    private fun records(store: JSONObject) = store.getJSONArray("connections")
    private fun find(id: String): JSONObject {
        val rows = records(readStore())
        for (i in 0 until rows.length()) if(rows.getJSONObject(i).getString("id")==id) return rows.getJSONObject(i)
        fail(texts.get("mobile.native.connectionMissing"))
    }
    private fun port(row: JSONObject, name: String): Int { val n=row.optInt(name,0); if(n !in 1..65535) fail(texts.get("mobile.native.portRange")); return n }
    private fun validate(row: JSONObject) {
        if(row.optString("name").isBlank()) fail(texts.get("mobile.native.nameRequired"))
        when(row.optString("mode")) {
            "url" -> safeUrl(row.getString("url"))
            "ssh" -> {
                if(row.optString("host").isBlank() || row.getString("host").any { it.isWhitespace() || it=='/' || it=='@' }) fail(texts.get("mobile.native.sshHostNameInvalid"))
                if(row.optString("username").isBlank()) fail(texts.get("mobile.native.sshUsernameRequired"))
                port(row,"port")
                if(row.optString("auth")=="key") { if(row.optString("privateKey").isBlank()) fail(texts.get("mobile.native.privateKeyRequired")) }
                else if(row.optString("auth")!="password" || row.optString("password").isEmpty()) fail(texts.get("mobile.native.sshPasswordRequired"))
                if(row.optString("service")=="manual") port(row,"remotePort")
                else if(row.optString("service")!="auto") fail(texts.get("mobile.native.serviceModeRequired"))
            }
            else -> fail(texts.get("mobile.native.modeUnsupported"))
        }
    }
    private fun safeUrl(text: String): URI {
        val url=URI(text)
        if(url.scheme !in listOf("https","http") || url.host.isNullOrBlank() || url.userInfo!=null) fail(texts.get("mobile.native.addressInvalid"))
        if(url.scheme=="http" && url.host !in listOf("localhost","127.0.0.1","[::1]","::1")) fail(texts.get("mobile.native.httpsRequired"))
        return url
    }
    @PluginMethod fun list(call: PluginCall) = run(call) {
        val store = readStore(); val rows = ConnectionRecords.unique(records(store))
        if (rows.length() != records(store).length()) { store.put("connections", rows); writeStore(store) }
        val result = JSONArray(); for (i in 0 until rows.length()) result.put(ConnectionRecords.summary(rows.getJSONObject(i)))
        JSObject().put("connections", result)
    }
    @PluginMethod fun save(call: PluginCall) = run(call) {
        val store = readStore()
        val input = call.getObject("connection") ?: fail(texts.get("mobile.native.connectionConfigMissing"))
        val copyId = call.getString("copyFromId")
        val source = if (copyId != null) (0 until records(store).length()).map { records(store).getJSONObject(it) }.firstOrNull { it.optString("id") == copyId } else null
        if (copyId != null && (input.has("id") || source == null || source.optString("mode") != input.optString("mode"))) fail(texts.get("mobile.native.sourceConnectionMissing"))
        val row = if (source != null) ConnectionRecords.copied(input, source) else ConnectionRecords.prepared(input, records(store)); validate(row)
        if (copyId == null) input.optString("id").takeIf { it.isNotBlank() }?.let { PushRegistration.get(context).revoke(it) }
        val (saved, rows) = ConnectionRecords.upsert(row, records(store), preserveEquivalent = copyId != null)
        store.put("connections", rows); writeStore(store)
        JSObject().put("connection", ConnectionRecords.summary(saved))
    }
    private fun saveWebPassword(id: String, password: String) {
        val store = readStore(); val rows = records(store)
        val row = (0 until rows.length()).map { rows.getJSONObject(it) }.firstOrNull { it.getString("id") == id } ?: fail(texts.get("mobile.native.connectionMissing"))
        row.put("webPassword", password)
        store.put("connections", ConnectionRecords.upsert(row, rows).second); writeStore(store)
    }
    @PluginMethod fun remove(call: PluginCall) = run(call) {
        PushRegistration.get(context).revoke(call.getString("id") ?: "")
        val id=call.getString("id") ?: fail(texts.get("mobile.native.connectionIdMissing")); val store=readStore();val rows=records(store)
        for(i in rows.length()-1 downTo 0) if(rows.getJSONObject(i).getString("id")==id) rows.remove(i)
        writeStore(store); JSObject()
    }
    private val diagnosticLock = Any()
    private var diagnosticOperation = java.util.UUID.randomUUID().toString()
    private var diagnosticPhaseAt = System.nanoTime()
    private var diagnosticPreviousPhase = "idle"
    private fun state(phase: String) {
        synchronized(diagnosticLock) {
            val now = System.nanoTime()
            if (phase == "connecting") { diagnosticOperation = java.util.UUID.randomUUID().toString(); diagnosticPhaseAt = now }
            val safePhase = if (phase in setOf("connecting", "confirming", "preparing", "forwarding", "ready", "error", "disconnected")) phase else "unknown"
            val timestamp = java.text.SimpleDateFormat("yyyy-MM-dd HH:mm:ss", java.util.Locale.US).format(java.util.Date())
            val level = if (safePhase == "error") "WARN " else "INFO "
            // Never include connection records, URLs, credentials or exception messages.
            android.util.Log.i("VelaTerm", "$timestamp [$level] [system] event=mobile_connection operationId=$diagnosticOperation step=$safePhase previousStep=$diagnosticPreviousPhase durationMs=${(now-diagnosticPhaseAt)/1_000_000}")
            diagnosticPhaseAt = now
            diagnosticPreviousPhase = safePhase
        }
        notifyListeners("state",JSObject().put("phase",phase))
    }
    private fun approve(host: String, fingerprint: String, changed: Boolean): Boolean {
        val epoch = generation.get()
        val latch=CountDownLatch(1); var accepted=false
        main.post {
            if (generation.get() != epoch) { latch.countDown(); return@post }
            AlertDialog.Builder(activity).setTitle(texts.get(if(changed) "mobile.native.trustChangedTitle" else "mobile.native.trustTitle"))
                .setMessage(texts.get(if(changed) "mobile.native.trustChangedBody" else "mobile.native.trustBody", mapOf("identity" to host, "fingerprint" to fingerprint)))
                .setPositiveButton(texts.get("mobile.native.trustAccept")) { _,_ -> accepted=true; latch.countDown() }
                .setNegativeButton(texts.get("common.cancel")) { _,_ -> latch.countDown() }.setOnCancelListener { latch.countDown() }.show()
        }
        return latch.await(90,TimeUnit.SECONDS) && accepted
    }
    private fun exec(command: String, ssh: SSHClient): String {
        ssh.startSession().use { session ->
            val cmd=session.exec(command)
            val collected=java.io.ByteArrayOutputStream();val chunk=ByteArray(8192)
            while(true) {val count=cmd.inputStream.read(chunk);if(count<0)break;collected.write(chunk,0,count);if(collected.size()>1024*1024)fail(texts.get("mobile.native.responseTooLarge"))}
            val output=collected.toByteArray()
            cmd.join(180,TimeUnit.SECONDS)
            return String(output,StandardCharsets.UTF_8).trim()
        }
    }
    private fun resource(name: String)=context.assets.open(name).bufferedReader().use { it.readText() }
    // download.js is shared with the page context and reads its texts from this object instead of carrying them itself.
    private fun downloadTexts(strings: MobileText)="window.__VELATERM_DOWNLOAD_TEXT__="+JSONObject().put("mobile.native.downloadRetry",strings.get("mobile.native.downloadRetry")).put("mobile.native.downloadTooLarge",strings.get("mobile.native.downloadTooLarge")).toString()+";"
    private fun powershell(script: String)="powershell.exe -NoProfile -NonInteractive -EncodedCommand "+Base64.encodeToString(script.toByteArray(Charsets.UTF_16LE),Base64.NO_WRAP)
    private fun allocate(row: JSONObject): ServerSocket {
        var preferred=row.optInt("localPort",0)
        repeat(100) {
            val p=if(preferred in 10000..49151) preferred else SecureRandom().nextInt(39152)+10000
            try {
                val socket=ServerSocket(p,50,InetAddress.getByName("127.0.0.1"))
                val store=readStore(); val rows=records(store)
                for(i in 0 until rows.length()) if(rows.getJSONObject(i).getString("id")==row.getString("id")) rows.getJSONObject(i).put("localPort",p)
                writeStore(store); return socket
            } catch(e: java.net.BindException) { preferred=0 }
        }
        fail(texts.get("mobile.native.localPortFailed"))
    }
    private fun establish(row: JSONObject, epoch: Int): Pair<String,String> {
        if(row.getString("mode")=="url") return row.getString("url") to row.optString("webPassword")
        state("connecting")
        synchronized(net.schmizz.sshj.common.SecurityUtils::class.java) {
            if(java.security.Security.getProvider("BC") !is org.bouncycastle.jce.provider.BouncyCastleProvider) {
                java.security.Security.removeProvider("BC")
                java.security.Security.addProvider(org.bouncycastle.jce.provider.BouncyCastleProvider())
            }
            net.schmizz.sshj.common.SecurityUtils.setSecurityProvider("BC")
        }
        val ssh=SSHClient()
        synchronized(this) {
            if (generation.get() != epoch) { ssh.close(); fail(texts.get("mobile.native.connectionCancelled")) }
            client=ssh
        }
        ssh.connectTimeout=20000; ssh.timeout=180000
        fun execute(command: String): String {
            if (generation.get() != epoch) fail(texts.get("mobile.native.connectionCancelled"))
            return exec(command, ssh)
        }
        val host=row.getString("host");val remoteSshPort=port(row,"port");val identity="$host:$remoteSshPort"
        ssh.addHostKeyVerifier(object : net.schmizz.sshj.transport.verification.HostKeyVerifier {
            override fun findExistingAlgorithms(hostname:String,port:Int):List<String> = emptyList()
            override fun verify(hostname:String,port:Int,key:java.security.PublicKey):Boolean {
            val buffer=Buffer.PlainBuffer();buffer.putPublicKey(key)
            val digest=MessageDigest.getInstance("SHA-256").digest(buffer.compactData)
            val fingerprint="SHA256:"+Base64.encodeToString(digest,Base64.NO_WRAP or Base64.NO_PADDING)
            val store=readStore();val keys=store.getJSONObject("keys");val prior=keys.optString(identity)
            return if(prior==fingerprint) true else {
                state("confirming")
                if(generation.get() !=epoch || !approve(identity,fingerprint,prior.isNotEmpty()) || generation.get() !=epoch) false
                else { keys.put(identity,fingerprint);writeStore(store);true }
            }
            }
        })
        ssh.connect(host,remoteSshPort)
        if(generation.get() !=epoch) fail(texts.get("mobile.native.connectionCancelled"))
        if(row.getString("auth")=="key") {
            val provider=OpenSSHKeyV1KeyFile()
            val pass=row.optString("passphrase")
            provider.init(row.getString("privateKey"),null, if(pass.isEmpty()) null else PasswordUtils.createOneOff(pass.toCharArray()))
            ssh.authPublickey(row.getString("username"),provider)
        } else ssh.authPassword(row.getString("username"),row.getString("password"))
        state("preparing")
        var servicePort=row.optInt("remotePort",0); var password=row.optString("webPassword")
        if(row.getString("service")=="auto") {
            val windows= !execute("uname -s").contains(Regex("Linux|Darwin|MINGW|CYGWIN"))
            val discover=JSONObject(resource("discover.json"))
            var record=try { JSONObject(execute(if(windows) powershell(discover.getString("windows")) else discover.getString("posix"))) } catch(_: Exception) { JSONObject() }
            if(!record.has("port") || row.optBoolean("prepare")) {
                val code=resource("bootstrap-code.txt").trim()
                val command="${if(windows) "python" else "python3"} -c \"$code\"${if(row.optBoolean("prepare")) " --install" else ""}"
                val output=execute(command)
                record=try { JSONObject(output) } catch(_: Exception) { fail(texts.get("mobile.native.pythonRequired")) }
                if(record.has("error")) fail(record.getString("error"))
            }
            servicePort=port(record,"port");password=record.getString("password")
        }
        if(generation.get() !=epoch) fail(texts.get("mobile.native.connectionCancelled"))
        state("forwarding")
        val socket=allocate(row)
        synchronized(this) {
            if (generation.get() != epoch) { socket.close(); fail(texts.get("mobile.native.connectionCancelled")) }
            listener=socket
        }
        val forwarder=ssh.newLocalPortForwarder(Parameters("127.0.0.1",socket.localPort,"127.0.0.1",servicePort),socket)
        Thread({try { forwarder.listen() } catch(_:Exception) { if(generation.get() ==epoch) state("disconnected") }},"vela-ssh-forward").start()
        val url="http://127.0.0.1:${socket.localPort}"
        val health=java.net.URL("$url/api/mode").openConnection().apply {connectTimeout=10000;readTimeout=10000}
        health.getInputStream().use { it.read() }
        return url to password
    }
    private class AccountFailure(val code: String, message: String) : IllegalArgumentException(message)
    private var accountAttempt: JSONObject? = null
    private fun accountRequest(path: String, token: String? = null, body: JSONObject? = null): Any {
        val connection = java.net.URL("https://velaterm.com$path").openConnection() as javax.net.ssl.HttpsURLConnection
        try {
            connection.connectTimeout=15000;connection.readTimeout=40000;connection.instanceFollowRedirects=false
            token?.let {connection.setRequestProperty("Authorization","Bearer $it")}
            if(body != null) {connection.requestMethod="POST";connection.doOutput=true;connection.setRequestProperty("Content-Type","application/json");connection.outputStream.use {it.write(body.toString().toByteArray(StandardCharsets.UTF_8))}}
            val status=connection.responseCode
            if(path.endsWith("/poll") && status in listOf(401,404)) throw AccountFailure("ACCOUNT_LOGIN_EXPIRED",texts.get("mobile.native.loginRequestExpired"))
            if(status==401) throw AccountFailure("ACCOUNT_AUTH_REQUIRED",texts.get("mobile.native.sessionExpired"))
            if(status !in 200..299) fail(texts.get("mobile.native.accountServiceUnavailable"))
            val text=connection.inputStream.bufferedReader().use {it.readText()}
            return if(text.isBlank() || text=="null") JSONObject.NULL else org.json.JSONTokener(text).nextValue()
        } finally {connection.disconnect()}
    }
    private fun accountPage(address: String) {
        val uri=android.net.Uri.parse(address)
        if(uri.scheme!="https" || uri.host!="velaterm.com" || uri.port!=-1 || uri.userInfo!=null) fail(texts.get("mobile.native.accountAddressInvalid"))
        androidx.browser.customtabs.CustomTabsIntent.Builder().setShowTitle(true).build().launchUrl(activity,uri)
    }
    private fun openAccountPage(address: String) {
        val task=java.util.concurrent.FutureTask<Unit> {accountPage(address)}
        main.post(task);task.get(15,java.util.concurrent.TimeUnit.SECONDS)
    }
    @PluginMethod fun account(call: PluginCall) = run(call) {
        val store=readStore();val token=store.optString("accountToken").takeIf {it.isNotBlank()}
        if(accountAttempt==null) accountAttempt=store.optJSONObject("accountAttempt")
        when(call.getString("action") ?: "status") {
            "login" -> {
                val key=org.bouncycastle.crypto.params.X25519PrivateKeyParameters(java.security.SecureRandom())
                val input=JSONObject().put("name",android.os.Build.MODEL).put("publicKey",Base64.encodeToString(key.generatePublicKey().encoded,Base64.NO_WRAP))
                val attempt=accountRequest("/api/device-link",body=input) as JSONObject
                accountAttempt=attempt
                store.put("accountAttempt",attempt);writeStore(store)
                try {openAccountPage(attempt.getString("url"))} catch(error:Exception) {accountAttempt=null;store.remove("accountAttempt");writeStore(store);throw error}
                JSObject().put("linked",false)
            }
            "poll" -> {
                val attempt=accountAttempt ?: throw AccountFailure("ACCOUNT_LOGIN_EXPIRED",texts.get("mobile.native.loginRestart"))
                val code=attempt.getString("code");if(!code.matches(Regex("[A-Za-z0-9_-]{43}"))) fail(texts.get("mobile.native.loginRequestInvalid"))
                val value=accountRequest("/api/device-link/$code/poll",attempt.getString("pollToken"),JSONObject())
                if(value is JSONObject) {store.put("accountToken",value.getString("token"));store.remove("accountAttempt");writeStore(store);accountAttempt=null;JSObject().put("linked",true)}
                else JSObject().put("linked",false)
            }
            "status" -> (if(token==null) JSObject().put("linked",false) else JSObject((accountRequest("/api/device-link/host/status",token) as JSONObject).toString())).put("pending",accountAttempt!=null)
            "devices" -> JSObject().put("devices",accountRequest("/api/device-link/host/remote",token ?: fail(texts.get("mobile.native.signInFirst"))))
            "logout" -> {PushRegistration.get(context).revokeAccount();if(token!=null && (accountRequest("/api/device-link/host/status",token) as JSONObject).optBoolean("linked")) accountRequest("/api/device-link/host/logout",token,JSONObject());store.remove("accountToken");store.remove("accountAttempt");writeStore(store);accountAttempt=null;JSObject()}
            "open" -> {
                val body=JSONObject();call.getString("deviceId")?.let {body.put("deviceId",java.util.UUID.fromString(it).toString())}
                call.getString("grantId")?.let {body.put("grantId",java.util.UUID.fromString(it).toString())}
                val value=accountRequest("/api/device-link/host/browser-ticket",token ?: fail(texts.get("mobile.native.signInFirst")),body) as JSONObject
                if (body.has("deviceId") || body.has("grantId")) {
                    val address = value.getString("url"); val uri = URI(address)
                    check(uri.scheme == "https" && uri.host == "velaterm.com" && uri.port == -1 && uri.userInfo == null)
                    val open = java.util.concurrent.FutureTask<Unit> {
                        activeId = "account"; requestedAccountSession = call.getString("sessionId")
                        showBrowser(address, "", "VelaTerm")
                    }
                    main.post(open); open.get(15, TimeUnit.SECONDS)
                } else openAccountPage(value.getString("url"))
                JSObject()
            }
            else -> fail(texts.get("mobile.native.accountActionInvalid"))
        }
    }
    @PluginMethod fun connect(call: PluginCall) {
        val id = call.getString("id") ?: run { call.reject(texts.get("mobile.native.connectionIdMissing")); return }
        val epoch = generation.incrementAndGet(); activeId = id
        closeTransportInBackground()
        run(call) {
        try {
            if (generation.get() != epoch) fail(texts.get("mobile.native.connectionCancelled"))
            val row=find(id);val (address,password)=establish(row,epoch)
            val session = call.getString("sessionId")?.takeIf { it.isNotEmpty() && it.toByteArray().size <= 256 }
            val url = if (session == null) address else {
                val uri = android.net.Uri.parse(address)
                val builder = uri.buildUpon().clearQuery()
                for (key in uri.queryParameterNames) if (key != "session") for (value in uri.getQueryParameters(key)) builder.appendQueryParameter(key, value)
                builder.appendQueryParameter("session", session).build().toString()
            }
            if(generation.get() !=epoch) fail(texts.get("mobile.native.connectionCancelled"))
            main.post { if (generation.get() == epoch) showBrowser(url,password,row.getString("name")) }
            state("ready");JSObject().put("id",id)
        } catch(e:Exception) { if (generation.get() == epoch) { closeTransportInBackground(epoch);state("error") };throw e }
        }
    }
    @Synchronized private fun detachTransport(epoch: Int? = null): () -> Unit {
        if (epoch != null && generation.get() != epoch) return {}
        val oldListener = listener; listener = null
        val oldClient = client; client = null
        return {
            try { oldListener?.close() } catch (_: Exception) {}
            try { oldClient?.close() } catch (_: Exception) {}
        }
    }
    private fun closeTransport() { detachTransport().invoke() }
    private fun closeTransportInBackground(epoch: Int? = null) { val cleanup = detachTransport(epoch); cleanupWorker.execute { cleanup() } }
    @PluginMethod fun disconnect(call: PluginCall) {
        generation.incrementAndGet(); activeId = null; closeTransportInBackground()
        main.post { browser?.dismiss(); state("disconnected"); call.resolve() }
    }
    @PluginMethod fun status(call: PluginCall) { call.resolve(JSObject().put("connected",client?.isConnected ?: (web!=null)).put("id",activeId)) }
    private fun showBrowser(address: String,password: String,title: String) {
        val previous=try {web?.url?.let {URI(it)}} catch(_:Exception){null}
        val oldWeb=web;web=null;browser?.setOnDismissListener(null);browser?.dismiss();oldWeb?.stopLoading();oldWeb?.destroy()
        val uri=safeUrl(address); val origin="${uri.scheme}://${uri.rawAuthority}"
        val pageAddress=if(previous?.host==uri.host && previous.scheme==uri.scheme) URI(uri.scheme,null,uri.host,uri.port,previous.path,previous.query,previous.fragment).toString() else address
        val view=WebView(activity);web=view
        val connectionId = activeId
        view.settings.javaScriptEnabled=true;view.settings.domStorageEnabled=true
        view.addJavascriptInterface(object {
            @android.webkit.JavascriptInterface fun save(name:String,encoded:String) {
                if(encoded.length>89478488 || !downloadPending.compareAndSet(false,true))return
                try {
                    val bytes=Base64.decode(encoded,Base64.DEFAULT)
                    if(bytes.size>64*1024*1024)throw IllegalArgumentException()
                    main.post {
                        if(web!==view){downloadPending.set(false);return@post}
                        downloadBytes=bytes;pickingFile=true
                        val filename=name.substringAfterLast('/').substringAfterLast('\\').take(180).ifBlank{"download"}
                        try {downloadChooser?.launch(android.content.Intent(android.content.Intent.ACTION_CREATE_DOCUMENT).apply {
                            addCategory(android.content.Intent.CATEGORY_OPENABLE);type="application/octet-stream";putExtra(android.content.Intent.EXTRA_TITLE,filename)
                        })} catch(_:Exception){downloadBytes=null;downloadPending.set(false);pageStatus?.text=texts.get("mobile.native.savePickerFailed")}
                    }
                } catch(_:Exception){downloadPending.set(false)}
            }
        },"VelaFileExport")
        view.setDownloadListener { url,_,_,_,_ ->
            view.evaluateJavascript("window.__velaSaveDownload?.(${JSONObject.quote(url)},'download')",null)
        }
        view.settings.allowFileAccess=false;view.settings.allowContentAccess=false
        view.settings.setSupportMultipleWindows(false)
        view.settings.userAgentString += " VelaTermMobile/1"
        val layout=android.widget.FrameLayout(activity)
        val recoveryView = ConnectionRecoveryView(activity); recovery = recoveryView
        val strings = MobileText(activity)
        var navigationFailed = false
        var reconnecting = false
        var timeout: Runnable? = null
        fun beginLoading() {
            timeout?.let { main.removeCallbacks(it) }
            navigationFailed = false; recoveryView.showLoading()
            timeout = Runnable {
                if (web === view && !navigationFailed) {
                    reconnecting = false
                    recoveryView.showLoading(strings.get("mobile.loadSlow"), allowRetry = true)
                }
            }.also { main.postDelayed(it, 30000) }
        }
        fun showPageError(message: String) {
            if (web !== view) return
            timeout?.let { main.removeCallbacks(it) }; reconnecting = false
            navigationFailed = true; recoveryView.show(message)
            view.clearFocus()
            (activity.getSystemService(Context.INPUT_METHOD_SERVICE) as android.view.inputmethod.InputMethodManager).hideSoftInputFromWindow(view.windowToken, 0)
        }
        val reload=Button(activity).apply {text=strings.get("common.retry");setOnClickListener {
            val id = activeId ?: return@setOnClickListener
            if (id == "account") { beginLoading(); view.reload(); return@setOnClickListener }
            if (reconnecting || recovery !== recoveryView) return@setOnClickListener
            reconnecting = true; view.stopLoading(); beginLoading()
            val epoch = generation.incrementAndGet()
            closeTransportInBackground()
            worker.execute {
                try {
                    if (generation.get() != epoch) return@execute
                    val row = find(id); val (next, secret) = establish(row, epoch)
                    main.post { if (generation.get() == epoch && recovery === recoveryView) showBrowser(next, secret, title) }
                } catch (_: Exception) {
                    if (generation.get() == epoch) closeTransportInBackground(epoch)
                    main.post { if (generation.get() == epoch && recovery === recoveryView) showPageError(strings.get("mobile.connectionUnavailable")) }
                }
            }
        } }
        reconnectPage = { reload.performClick() }
        recoveryView.onRetry = { reload.performClick() }; recoveryView.onBack = { browser?.dismiss() }
        val status=TextView(activity).apply {text=title;textSize=15f;maxLines=1;ellipsize=android.text.TextUtils.TruncateAt.MIDDLE};pageStatus=status
        fun showConnectionPanel() {
            android.app.AlertDialog.Builder(activity).setTitle(strings.get("mobile.connections")).setMessage(if(status.text.toString()==title) title else "$title\n${status.text}")
                .setPositiveButton(strings.get("mobile.native.reconnect")) { _,_ -> reload.performClick() }
                .setNeutralButton(strings.get("mobile.native.switchConnection")) { _,_ -> browser?.dismiss() }
                .setNegativeButton(strings.get("common.cancel"),null).show()
        }
        layout.addView(view,android.widget.FrameLayout.LayoutParams(-1,-1))
        if (connectionId == "account") {
            val height = (48 * activity.resources.displayMetrics.density).toInt()
            (view.layoutParams as android.widget.FrameLayout.LayoutParams).bottomMargin = height
            layout.addView(Button(activity).apply {
                text = strings.get("mobile.backConnections"); textSize = 16f
                setOnClickListener { if (web === view) browser?.dismiss() }
            }, android.widget.FrameLayout.LayoutParams(-1, height, android.view.Gravity.BOTTOM))
        }
        layout.addView(recoveryView,android.widget.FrameLayout.LayoutParams(-1,-1))
        view.webChromeClient=object:android.webkit.WebChromeClient() {
            override fun onShowFileChooser(v:WebView,callback:android.webkit.ValueCallback<Array<android.net.Uri>>,params:FileChooserParams):Boolean {
                fileResult?.onReceiveValue(null);fileResult=callback;pickingFile=true
                return try {chooser?.launch(params.createIntent());true} catch(_:Exception){fileResult?.onReceiveValue(null);fileResult=null;false}
            }
        }
        view.webViewClient=object:WebViewClient() {
            override fun onReceivedSslError(v:WebView,handler:android.webkit.SslErrorHandler,error:android.net.http.SslError) {
                val errorUri=try {URI(error.url)} catch(_:Exception){handler.cancel();return}
                if(errorUri.host!=uri.host || errorUri.scheme!="https" || (errorUri.port.takeIf{it>0} ?: 443)!=(uri.port.takeIf{it>0} ?: 443)){handler.cancel();return}
                val bytes=android.net.http.SslCertificate.saveState(error.certificate).getByteArray("x509-certificate") ?: run{handler.cancel();return}
                val fingerprint="SHA256:"+Base64.encodeToString(MessageDigest.getInstance("SHA-256").digest(bytes),Base64.NO_WRAP or Base64.NO_PADDING)
                val identity="tls:$origin"
                worker.execute {try {
                    val store=readStore();val keys=store.getJSONObject("keys");val previous=keys.optString(identity)
                    val trusted=previous==fingerprint || approve(strings.get("mobile.native.tlsIdentity", mapOf("identity" to origin)),fingerprint,previous.isNotEmpty())
                    if(trusted){keys.put(identity,fingerprint);writeStore(store)}
                    main.post {if(trusted && web===v)handler.proceed() else {handler.cancel();if(web===v)showPageError(strings.get("mobile.native.certificateRejected"))}}
                } catch(_:Exception){main.post{handler.cancel();if(web===v)showPageError(strings.get("mobile.native.certificateRejected"))}}}
            }
            override fun shouldOverrideUrlLoading(v:WebView,request:WebResourceRequest):Boolean {
                val next=request.url
                if(next.scheme=="velaterm-ui" && next.host=="close" && request.isForMainFrame) {browser?.dismiss();return true}
                if(next.scheme=="velaterm-ui" && next.host=="connections" && request.isForMainFrame) {showConnectionPanel();return true}
                val allowed=next.scheme==uri.scheme && next.host==uri.host && (if(next.port==-1) if(next.scheme=="https")443 else 80 else next.port)==(if(uri.port==-1) if(uri.scheme=="https")443 else 80 else uri.port)
                if(!allowed && request.isForMainFrame) status.text=strings.get("mobile.native.navigationBlocked", mapOf("host" to (next.host ?: next.scheme ?: "")))
                return !allowed
            }
            override fun onReceivedError(v:WebView,r:WebResourceRequest,e:WebResourceError) {
                if(r.isForMainFrame) { status.text=strings.get("mobile.connectionUnavailable"); showPageError(strings.get("mobile.native.pageLoadFailed")) }
            }
            override fun onReceivedHttpError(v: WebView, request: WebResourceRequest, response: android.webkit.WebResourceResponse) {
                if (request.isForMainFrame) showPageError(strings.get("mobile.native.pageUnavailable", mapOf("code" to response.statusCode.toString())))
            }
            override fun onRenderProcessGone(v: WebView, detail: android.webkit.RenderProcessGoneDetail): Boolean {
                showPageError(strings.get("mobile.native.pageTerminated"))
                layout.removeView(v); v.destroy(); if (web === v) web = null
                return true
            }
            override fun onPageStarted(v: WebView, url: String, favicon: android.graphics.Bitmap?) {
                if (web !== v) return
                beginLoading()
                val current = android.net.Uri.parse(url)
                val pending = requestedAccountSession
                if (activeId == "account" && pending != null && current.host == "velaterm.com" && current.pathSegments.firstOrNull() == "r") {
                    requestedAccountSession = null
                    if (pending.matches(Regex("[A-Za-z0-9_-]{1,128}"))) {
                        v.stopLoading(); v.loadUrl(current.buildUpon().appendQueryParameter("session", pending).build().toString())
                    }
                }
            }
        }
        val supportsPasswordStorage = WebViewFeature.isFeatureSupported(WebViewFeature.WEB_MESSAGE_LISTENER)
        if (supportsPasswordStorage) WebViewCompat.addWebMessageListener(view, "VelaPageReady", setOf(origin)) { _, message, source, mainFrame, _ ->
            if (!mainFrame || source.scheme != uri.scheme || source.host != uri.host || (source.port.takeIf { it > 0 } ?: if (source.scheme == "https") 443 else 80) != (uri.port.takeIf { it > 0 } ?: if (uri.scheme == "https") 443 else 80) || web !== view || navigationFailed) return@addWebMessageListener
            try {
                if (JSONObject(message.data ?: "").optBoolean("ready")) {
                    timeout?.let { main.removeCallbacks(it) }; timeout = null
                    recoveryView.visibility = android.view.View.GONE; reconnecting = false; status.text = title
                }
            } catch (_: Exception) { /* Keep native navigation available for an invalid readiness message. */ }
        }
        if (supportsPasswordStorage) WebViewCompat.addWebMessageListener(view, "VelaNotifications", setOf(origin)) { _, message, source, mainFrame, reply ->
            if (!mainFrame || source.scheme != uri.scheme || source.host != uri.host || (source.port.takeIf { it > 0 } ?: if (source.scheme == "https") 443 else 80) != (uri.port.takeIf { it > 0 } ?: if (uri.scheme == "https") 443 else 80) || web !== view || connectionId == null || activeId != connectionId) return@addWebMessageListener
            var requestId = 0
            try {
                val body = JSONObject(message.data ?: ""); requestId = body.getInt("id")
                val request = requestId
                val current = android.net.Uri.parse(view.url)
                val path = current.pathSegments
                val notificationId = if (connectionId == "account") {
                    check(current.host == "velaterm.com" && path.size >= 2 && path[0] == "r")
                    "account_" + UUID.fromString(path[1]).toString()
                } else connectionId
                val push = PushRegistration.get(context)
                when (body.getString("action")) {
                    "permission" -> notifications.permission(body.optBoolean("request", false)) { result -> reply.postMessage(JSONObject().put("id", request).put("result", result).toString()) }
                    "send" -> { if (!push.isBound(notificationId)) notifications.send(notificationId, body); reply.postMessage(JSONObject().put("id", request).put("result", true).toString()) }
                    "subscription", "bound" -> worker.execute {
                        try {
                            val value: Any = if (body.getString("action") == "subscription") push.subscription(notificationId, if (notificationId.startsWith("account_")) readStore().optString("accountToken").takeIf { it.isNotBlank() } else null)
                                else { push.bound(notificationId, body.optBoolean("active")); true }
                            main.post { if (web === view && view.url == current.toString()) reply.postMessage(JSONObject().put("id", request).put("result", value).toString()) }
                        } catch (error: Exception) {
                            main.post { if (web === view) reply.postMessage(JSONObject().put("id", request).put("error", error.message?.takeIf {it.matches(Regex("PUSH_[A-Z_]+"))} ?: "PUSH_RELAY_UNAVAILABLE").toString()) }
                        }
                    }
                    else -> error("Unknown notification action")
                }
            } catch (_: Exception) { reply.postMessage(JSONObject().put("id", requestId).put("error", "Notifications unavailable").toString()) }
        }
        if (supportsPasswordStorage) WebViewCompat.addWebMessageListener(view, "VelaPassword", setOf(origin)) { _, message, source, mainFrame, reply ->
            if (!mainFrame || source.scheme != uri.scheme || source.host != uri.host || (source.port.takeIf { it > 0 } ?: if (source.scheme == "https") 443 else 80) != (uri.port.takeIf { it > 0 } ?: if (uri.scheme == "https") 443 else 80) || web !== view) return@addWebMessageListener
            try {
                val body = JSONObject(message.data ?: "")
                val requestId = body.getInt("id")
                val value = body.getString("password")
                if (value.toByteArray(Charsets.UTF_8).size > 4096) throw IllegalArgumentException()
                worker.execute {
                    val ok = try {
                        if (connectionId == null || activeId != connectionId || web !== view) fail(texts.get("mobile.native.connectionClosed"))
                        saveWebPassword(connectionId, value); true
                    } catch (_: Exception) { false }
                    main.post { if (web === view) reply.postMessage(JSONObject().put("id", requestId).put("ok", ok).toString()) }
                }
            } catch (_: Exception) { /* Invalid requests cannot write credentials. */ }
        }
        beginLoading()
        if(!supportsPasswordStorage || !WebViewFeature.isFeatureSupported(WebViewFeature.DOCUMENT_START_SCRIPT)) { state("error");showPageError(strings.get("mobile.native.webViewOutdated")) }
        else {
            val script=if(password.isNotEmpty()) "window.__VLX_AUTOLOGIN__={password:${JSONObject.quote(password)}};" else ""
            WebViewCompat.addDocumentStartJavaScript(view,"window.__VELATERM_CONNECTION_MENU__=true;"+script+downloadTexts(strings)+resource("download.js")+if (supportsPasswordStorage) resource("login.js")+"\n"+resource("notifications.js")+"\n"+resource("page-readiness.js") else "",setOf(origin))
            view.loadUrl(pageAddress)
        }
        val dialog=Dialog(activity,android.R.style.Theme_Material_Light_NoActionBar)
        dialog.setContentView(layout);dialog.setOnDismissListener {if(browser===dialog){timeout?.let { main.removeCallbacks(it) };if(web===view){view.stopLoading();view.destroy();web=null};recovery=null;browser=null;reconnectPage=null;activeId=null;generation.incrementAndGet();closeTransportInBackground();state("disconnected")}}
        browser=dialog;dialog.show();dialog.window?.setSoftInputMode(android.view.WindowManager.LayoutParams.SOFT_INPUT_ADJUST_RESIZE)
        androidx.core.view.ViewCompat.setOnApplyWindowInsetsListener(layout) { v,insets ->
            val bars=insets.getInsets(androidx.core.view.WindowInsetsCompat.Type.systemBars())
            val keyboardVisible=insets.isVisible(androidx.core.view.WindowInsetsCompat.Type.ime())
            v.setPadding(bars.left,bars.top,bars.right,if(keyboardVisible) 0 else bars.bottom);insets
        }
        androidx.core.view.ViewCompat.requestApplyInsets(layout)
    }
    override fun handleOnPause() { suspended=true;super.handleOnPause() }
    private fun consumeNotification(intent: android.content.Intent?) {
        val id = intent?.getStringExtra("velaNotificationConnection") ?: return
        val session = intent.getStringExtra("velaNotificationSession") ?: ""
        val event = intent.getStringExtra("velaNotificationEvent")
        intent.removeExtra("velaNotificationConnection"); intent.removeExtra("velaNotificationSession")
        intent.removeExtra("velaNotificationEvent"); intent.removeExtra("velaNotificationSubscription")
        if (event != null) {
            if (runCatching { UUID.fromString(event) }.isFailure) return
            val preferences = context.getSharedPreferences("vela-notification-clicks", Context.MODE_PRIVATE)
            val seen = JSONArray(preferences.getString("opened", "[]"))
            if ((0 until seen.length()).any { seen.optString(it) == event }) return
            seen.put(event)
            val retained = JSONArray(); for (index in maxOf(0, seen.length() - 200) until seen.length()) retained.put(seen.getString(index))
            preferences.edit().putString("opened", retained.toString()).apply()
        }
        if (id.length <= 256 && session.length <= 256) notifyListeners("notificationOpen", JSObject().put("id", id).put("sessionId", session), true)
    }
    override fun handleOnNewIntent(intent: android.content.Intent) { super.handleOnNewIntent(intent); consumeNotification(intent) }
    override fun handleOnResume() {
        super.handleOnResume()
        PushRegistration.get(context).resume()
        if(!suspended)return
        suspended=false
        if(pickingFile){pickingFile=false;return}
        if (activeId != null) reconnectPage?.invoke()
    }
    override fun handleOnDestroy() { notifications.close(); scanner?.unregister();scanCall?.reject(texts.get("mobile.native.scanCancelled"), "SCAN_UNAVAILABLE");scanCall=null; generation.incrementAndGet();closeTransportInBackground();cleanupWorker.shutdown();fileResult?.onReceiveValue(null);chooser?.unregister();downloadChooser?.unregister();downloadBytes=null;worker.shutdownNow();super.handleOnDestroy() }
}
