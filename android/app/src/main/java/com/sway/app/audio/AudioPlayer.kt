package com.sway.app.audio

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.util.Log
import java.nio.ByteBuffer
import java.nio.ByteOrder

class AudioPlayer {
    private val tag = "SwayAudioPlayer"

    private var audioTrack: AudioTrack? = null
    private var currentSampleRate = 48000
    private var currentChannels = 2
    private var isPlaying = false

    @Synchronized
    fun playPcmChunk(packet: ByteArray) {
        if (packet.size < 12) return

        // Verify magic SWAU header
        if (packet[0] != 'S'.code.toByte() ||
            packet[1] != 'W'.code.toByte() ||
            packet[2] != 'A'.code.toByte() ||
            packet[3] != 'U'.code.toByte()
        ) {
            return
        }

        val bb = ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN)
        bb.position(4)
        val channels = bb.short.toInt()
        val sampleRate = bb.int
        val bitsPerSample = bb.short.toInt()

        if (sampleRate <= 0 || channels <= 0) return

        val pcmDataOffset = 12
        val pcmDataLen = packet.size - pcmDataOffset
        if (pcmDataLen <= 0) return

        ensureAudioTrack(sampleRate, channels, bitsPerSample)

        try {
            audioTrack?.let { track ->
                if (track.playState != AudioTrack.PLAYSTATE_PLAYING) {
                    track.play()
                }
                track.write(packet, pcmDataOffset, pcmDataLen, AudioTrack.WRITE_NON_BLOCKING)
            }
        } catch (e: Exception) {
            Log.e(tag, "Error writing PCM audio data to AudioTrack", e)
        }
    }

    @Synchronized
    private fun ensureAudioTrack(sampleRate: Int, channels: Int, bitsPerSample: Int) {
        if (audioTrack != null && currentSampleRate == sampleRate && currentChannels == channels) {
            return
        }

        release()

        currentSampleRate = sampleRate
        currentChannels = channels

        val channelConfig = if (channels == 1) {
            AudioFormat.CHANNEL_OUT_MONO
        } else {
            AudioFormat.CHANNEL_OUT_STEREO
        }

        val audioEncoding = if (bitsPerSample == 32) {
            AudioFormat.ENCODING_PCM_FLOAT
        } else {
            AudioFormat.ENCODING_PCM_16BIT
        }

        val minBufferSize = AudioTrack.getMinBufferSize(sampleRate, channelConfig, audioEncoding)
        val bufferSize = (minBufferSize * 2).coerceAtLeast(4096)

        try {
            audioTrack = AudioTrack.Builder()
                .setAudioAttributes(
                    AudioAttributes.Builder()
                        .setUsage(AudioAttributes.USAGE_MEDIA)
                        .setContentType(AudioAttributes.CONTENT_TYPE_MUSIC)
                        .build()
                )
                .setAudioFormat(
                    AudioFormat.Builder()
                        .setEncoding(audioEncoding)
                        .setSampleRate(sampleRate)
                        .setChannelMask(channelConfig)
                        .build()
                )
                .setBufferSizeInBytes(bufferSize)
                .setTransferMode(AudioTrack.MODE_STREAM)
                .build()

            isPlaying = true
            Log.i(tag, "AudioTrack initialized: ${channels}ch @ ${sampleRate}Hz, bufferSize=$bufferSize")
        } catch (e: Exception) {
            Log.e(tag, "Failed to create AudioTrack", e)
            audioTrack = null
        }
    }

    @Synchronized
    fun stop() {
        try {
            audioTrack?.pause()
            audioTrack?.flush()
        } catch (e: Exception) {
            Log.w(tag, "Error stopping AudioTrack", e)
        }
    }

    @Synchronized
    fun release() {
        try {
            audioTrack?.stop()
            audioTrack?.release()
        } catch (e: Exception) {
            Log.w(tag, "Error releasing AudioTrack", e)
        } finally {
            audioTrack = null
            isPlaying = false
        }
    }
}
