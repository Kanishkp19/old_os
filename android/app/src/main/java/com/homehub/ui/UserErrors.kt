package com.homehub.ui

import android.content.Context
import com.homehub.R
import com.homehub.net.HubClient

object UserErrors {
    fun message(context: Context, error: Throwable): String = context.getString(when (error) {
        is HubClient.ApiException -> when(error.code) {
            401 -> R.string.error_reconnect
            403 -> R.string.error_permission
            404, 410 -> R.string.error_missing
            409 -> R.string.error_conflict
            507 -> R.string.error_storage
            else -> R.string.error_connection
        }
        is SecurityException -> R.string.error_access
        is java.io.FileNotFoundException -> R.string.error_missing
        is java.security.cert.CertificateException -> R.string.error_identity
        is java.io.IOException -> R.string.error_connection
        else -> R.string.error_operation
    })
}
