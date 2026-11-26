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
