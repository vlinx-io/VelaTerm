package com.velaterm.remote

import android.content.Context

/** Google Play builds ship without the Getui and vendor push SDKs, so task notifications report as not configured. */
internal object PushProvider {
    const val AVAILABLE = false
    fun initialize(context: Context) { error("PUSH_NOT_CONFIGURED") }
    fun clientId(context: Context): String? = null
    fun turnOff(context: Context) {}
}
