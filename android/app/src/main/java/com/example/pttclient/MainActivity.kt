package com.example.pttclient

import android.app.Activity
import android.os.Bundle
import android.view.Gravity
import android.widget.TextView

class MainActivity : Activity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)

        val icon = TextView(this).apply {
            text = "👅"
            textSize = 144f
            gravity = Gravity.CENTER
            setBackgroundColor(getColor(R.color.light_pink))
            contentDescription = getString(R.string.tongue_description)
            setOnClickListener {
                text = "💦"
                contentDescription = getString(R.string.water_drops_description)
            }
        }

        setContentView(icon)
    }
}
