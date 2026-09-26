package com.velaterm.remote

import android.content.Context
import com.igexin.sdk.PushManager

/** Getui and vendor channels for builds distributed outside Google Play. */
internal object PushProvider {
    const val AVAILABLE = true
    fun initialize(context: Context) {
        if (context.assets.list("")?.contains("agconnect-services.json") == true) {
            context.assets.open("agconnect-services.json").use {
                com.huawei.agconnect.AGConnectInstance.initialize(context, com.huawei.agconnect.AGConnectOptionsBuilder().setInputStream(it))
            }
        }
        PushManager.getInstance().initialize(context, VelaPushService::class.java)
        PushManager.getInstance().registerPushIntentService(context, VelaPushReceiver::class.java)
    }
    fun clientId(context: Context): String? = PushManager.getInstance().getClientid(context)
    fun turnOff(context: Context) { PushManager.getInstance().turnOffPush(context) }
}
