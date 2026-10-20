package com.sway.app

import android.app.Application
import com.sway.app.util.NotificationHelper

class SwayApplication : Application() {
    override fun onCreate() {
        super.onCreate()
        NotificationHelper.initChannels(this)
    }
}
