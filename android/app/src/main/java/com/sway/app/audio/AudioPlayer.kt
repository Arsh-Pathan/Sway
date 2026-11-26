package com.sway.app.audio

import android.media.AudioAttributes
import android.media.AudioFormat
import android.media.AudioTrack
import android.util.Log
import kotlinx.coroutines.CoroutineScope
import kotlinx.coroutines.Dispatchers
import kotlinx.coroutines.ExperimentalCoroutinesApi
import kotlinx.coroutines.Job
import kotlinx.coroutines.SupervisorJob
import kotlinx.coroutines.channels.BufferOverflow
import kotlinx.coroutines.channels.Channel
import kotlinx.coroutines.launch
import java.nio.ByteBuffer
import java.nio.ByteOrder
import java.util.concurrent.atomic.AtomicBoolean

@OptIn(ExperimentalCoroutinesApi::class)
class AudioPlayer {
    private val tag = "SwayAudioPlayer"

    private val audioDispatcher = Dispatchers.IO.limitedParallelism(1)
    private var scope: CoroutineScope? = null
    private var workerJob: Job? = null
    private var chunkChannel: Channel<ByteArray>? = null
    private val isEnabled = AtomicBoolean(false)

    private var audioTrack: AudioTrack? = null
    private var currentSampleRate = 0
    private var currentChannels = 0
    private var currentBitsPerSample = 0

    private fun ensureWorker() {
        isEnabled.set(true)
        if (workerJob?.isActive == true && chunkChannel != null) return

        val ch = Channel<ByteArray>(capacity = 6, onBufferOverflow = BufferOverflow.DROP_OLDEST)
        val newScope = CoroutineScope(SupervisorJob() + audioDispatcher)
        chunkChannel = ch
        scope = newScope
        workerJob = newScope.launch {
            for (packet in ch) {
                if (!isEnabled.get()) continue
                writeChunkInternal(packet)
            }
        }
    }

    fun playPcmChunk(packet: ByteArray) {
        if (packet.size < 12) return
        if (packet[0] != 'S'.code.toByte() ||
            packet[1] != 'W'.code.toByte() ||
            packet[2] != 'A'.code.toByte() ||
            packet[3] != 'U'.code.toByte()
        ) {
            return
        }
        ensureWorker()
        chunkChannel?.trySend(packet)
    }

    private fun writeChunkInternal(packet: ByteArray) {
        val bb = ByteBuffer.wrap(packet).order(ByteOrder.LITTLE_ENDIAN)
        bb.position(4)
        val channels = bb.short.toInt() and 0xFFFF
        val sampleRate = bb.int
        val bitsPerSample = bb.short.toInt() and 0xFFFF

        if (sampleRate <= 0 || channels <= 0 || (bitsPerSample != 16 && bitsPerSample != 32)) return

        val frameSize = channels * (bitsPerSample / 8)
        if (frameSize <= 0) return

        val pcmDataOffset = 12
        val rawLen = packet.size - pcmDataOffset
        val alignedLen = rawLen - (rawLen % frameSize)
        if (alignedLen <= 0) return

        ensureAudioTrack(sampleRate, channels, bitsPerSample)

        try {
            val track = audioTrack ?: return
            if (!isEnabled.get()) return
            if (track.playState != AudioTrack.PLAYSTATE_PLAYING) {
                track.play()
            }
            val byteBuf = ByteBuffer.wrap(packet, pcmDataOffset, alignedLen).order(ByteOrder.LITTLE_ENDIAN)
            val written = track.write(byteBuf, alignedLen, AudioTrack.WRITE_BLOCKING)
            if (written < 0) {
                Log.w(tag, "AudioTrack write returned status $written")
            }
        } catch (e: Exception) {
            Log.e(tag, "Error writing PCM audio data to AudioTrack", e)
        }
    }

    private fun ensureAudioTrack(sampleRate: Int, channels: Int, bitsPerSample: Int) {
        if (audioTrack != null &&
            currentSampleRate == sampleRate &&
            currentChannels == channels &&
            currentBitsPerSample == bitsPerSample
        ) {
            return
        }

        releaseTrackOnly()

        currentSampleRate = sampleRate
        currentChannels = channels
        currentBitsPerSample = bitsPerSample

        val channelConfig = when (channels) {
            1 -> AudioFormat.CHANNEL_OUT_MONO
            6 -> AudioFormat.CHANNEL_OUT_5POINT1
            8 -> AudioFormat.CHANNEL_OUT_7POINT1_SURROUND
            else -> AudioFormat.CHANNEL_OUT_STEREO
        }

        val audioEncoding = if (bitsPerSample == 32) {
            AudioFormat.ENCODING_PCM_FLOAT
        } else {
            AudioFormat.ENCODING_PCM_16BIT
        }

        val minBufferSize = AudioTrack.getMinBufferSize(sampleRate, channelConfig, audioEncoding)
        if (minBufferSize <= 0) {
            Log.e(tag, "Invalid AudioTrack buffer size $minBufferSize for ${channels}ch @ ${sampleRate}Hz ${bitsPerSample}bit")
            return
        }
        val bufferSize = (minBufferSize * 2).coerceAtLeast(minBufferSize)

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
                .setPerformanceMode(AudioTrack.PERFORMANCE_MODE_LOW_LATENCY)
                .build()

            Log.i(tag, "AudioTrack initialized: ${channels}ch @ ${sampleRate}Hz ${bitsPerSample}bit, bufferSize=$bufferSize")
        } catch (e: Exception) {
            Log.e(tag, "Failed to create AudioTrack", e)
            audioTrack = null
        }
    }

    fun stop() {
        isEnabled.set(false)
        val ch = chunkChannel
        if (ch != null) {
            while (ch.tryReceive().isSuccess) {
                // Drain queued packets
            }
        }
        scope?.launch {
            try {
                audioTrack?.pause()
                audioTrack?.flush()
            } catch (e: Exception) {
                Log.w(tag, "Error stopping AudioTrack", e)
            }
        }
    }

    private fun releaseTrackOnly() {
        try {
            audioTrack?.pause()
            audioTrack?.flush()
            audioTrack?.stop()
            audioTrack?.release()
        } catch (e: Exception) {
            Log.w(tag, "Error releasing AudioTrack", e)
        } finally {
            audioTrack = null
            currentSampleRate = 0
            currentChannels = 0
            currentBitsPerSample = 0
        }
    }

    fun release() {
        isEnabled.set(false)
        val oldCh = chunkChannel
        val oldJob = workerJob
        val oldScope = scope
        chunkChannel = null
        workerJob = null
        scope = null

        oldCh?.close()
        oldJob?.cancel()
        CoroutineScope(audioDispatcher).launch {
            releaseTrackOnly()
            oldScope?.let {
                try {
                    it.coroutineContext[Job]?.cancel()
                } catch (_: Exception) {}
            }
        }
    }
}
