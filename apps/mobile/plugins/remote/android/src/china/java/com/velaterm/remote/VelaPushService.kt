package com.velaterm.remote

import android.content.Context
import com.igexin.sdk.PushService
import com.igexin.sdk.GTIntentService

class VelaPushService : PushService()

class VelaPushReceiver : GTIntentService() {
    override fun onReceiveClientId(context: Context, clientId: String) {
        PushRegistration.get(context).token(clientId)
    }
}
