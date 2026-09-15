package com.velaterm.remote

import android.content.Context
import org.json.JSONObject
import java.util.Locale

/** Display strings are generated from the web dictionaries during resource synchronization. */
internal class MobileText(context: Context) {
    private val dictionaries = JSONObject(context.assets.open("native-text.json").bufferedReader().use { it.readText() })
    private val languages = context.resources.configuration.locales
    /** Resolves `key` for the device language; `{name}` placeholders are replaced with `values["name"]`. */
    fun get(key: String, values: Map<String, String> = emptyMap()): String =
        values.entries.fold(lookup(key)) { text, (name, value) -> text.replace("{$name}", value) }
    private fun lookup(key: String): String {
        for (index in 0 until languages.size()) {
            val language = languages[index].toLanguageTag().lowercase(Locale.ROOT)
            val locale = when {
                language.startsWith("zh") -> if (listOf("hant", "tw", "hk", "mo").any { language.contains(it) }) "zh-TW" else "zh-CN"
                language.startsWith("pt") -> "pt-BR"
                else -> language.substringBefore('-')
            }
            dictionaries.optJSONObject(locale)?.optString(key)?.takeIf { it.isNotEmpty() }?.let { return it }
        }
        return dictionaries.getJSONObject("en").getString(key)
    }
}
