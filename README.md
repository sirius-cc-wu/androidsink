# Androidsink

Android example for gstreamer-rs

# Build

必須先編譯安卓的 gstreamer，並且將其 pkgconfig 拿出來放到一個目錄下，並修正各個 .pc 檔的路徑，目錄結構如下：

* pkgconfig
  * armv7
  * arm64
  * x86
  * x86\_64

```
# Environment variables
export ANDROID_HOME=<path/to/android/home>
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/tools/bin:$ANDROID_HOME/build-tools/29.0.2:$ANDROID_HOME/platform-tools
export ANDROID_NDK_HOME=<path/to/ndk>
export PATH=$PATH:$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang
export CXX=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang++
export PKG_CONFIG_ALLOW_CROSS=1 
export GST_PKG_CONFIG=<path/to/android/gstreamers/pkgconfig>

# Rust library
cd examples/sink
./gradlew cargoBuild

# Copy all of the gstreamer libraries used into examples/sink/app/build/rustJniLibs

...

# App and rust library
cd examples/sink
./gradlew installDebug
```

# 文獻

* [Create a Basic Android App without an IDE ](https://developer.okta.com/blog/2018/08/10/basic-android-without-an-ide)
