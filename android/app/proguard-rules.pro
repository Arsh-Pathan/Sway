# Keep WebRTC native/JNI classes and callbacks
-keep class org.webrtc.** { *; }
-dontwarn org.webrtc.**

# Keep kotlinx.serialization models and serializers
-keepattributes *Annotation*, InnerClasses
-keepclassmembers class kotlinx.serialization.json.** { *** Companion; }
-keepclasseswithmembers class kotlinx.serialization.json.** { kotlinx.serialization.KSerializer serializer(...); }
-keep,includedescriptorclasses class com.sway.app.model.**$$serializer { *; }
-keepclassmembers class com.sway.app.model.** { *** Companion; }
-keep class com.sway.app.model.** { *; }

# Keep MLKit & ODML classes loaded via reflection/native JNI
-keep class com.google.mlkit.** { *; }
-dontwarn com.google.mlkit.**
-keep class com.google.android.odml.** { *; }
-dontwarn com.google.android.odml.**

# Suppress optional JVM/TLS platform warnings in OkHttp / Conscrypt
-dontwarn okhttp3.internal.platform.**
-dontwarn org.conscrypt.**
-dontwarn org.bouncycastle.**
-dontwarn org.openjsse.**
