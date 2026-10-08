package com.homehub.ui

import android.app.Activity
import android.content.Context
import android.content.res.Configuration
import android.os.Build
import android.os.LocaleList
import androidx.compose.foundation.layout.Column
import androidx.compose.material3.Text
import androidx.compose.material3.TextButton
import androidx.compose.runtime.Composable
import androidx.compose.ui.platform.LocalContext
import androidx.compose.ui.res.stringResource
import com.homehub.R
import java.util.Locale

object LanguagePreference {
    fun wrap(context: Context): Context {
        val tag = context.getSharedPreferences("homehub_ui", Context.MODE_PRIVATE).getString("language", "").orEmpty()
        if (tag.isBlank()) return context
        val config = Configuration(context.resources.configuration)
        config.setLocales(LocaleList.forLanguageTags(tag))
        return context.createConfigurationContext(config)
    }
    fun set(context: Context, tag: String) {
        require(tag in listOf("", "en", "hi"))
        require(context.getSharedPreferences("homehub_ui", Context.MODE_PRIVATE).edit().putString("language", tag).commit())
        if (Build.VERSION.SDK_INT >= 33) context.getSystemService(android.app.LocaleManager::class.java).applicationLocales = LocaleList.forLanguageTags(tag)
        val app = context.applicationContext
        val config = Configuration(app.resources.configuration)
        val locales = if (tag.isBlank()) android.content.res.Resources.getSystem().configuration.locales else LocaleList.forLanguageTags(tag)
        config.setLocales(locales)
        @Suppress("DEPRECATION") app.resources.updateConfiguration(config, app.resources.displayMetrics)
        (context as? Activity)?.recreate()
    }
}
@Composable
fun LanguageSettings() {
    val context = LocalContext.current
    Column {
        Text(stringResource(R.string.language_title))
        listOf("" to R.string.language_system, "en" to R.string.language_english, "hi" to R.string.language_hindi).forEach { (tag, label) ->
            TextButton(onClick = { LanguagePreference.set(context, tag) }) { Text(stringResource(label)) }
        }
    }
}
