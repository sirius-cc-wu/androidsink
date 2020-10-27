# Androidsink

Android example for gstreamer-rs

# Build

This project uses Mozilla's [rut-android-gradle](https://github.com/mozilla/rust-android-gradle). Before building the project the following environment variables should be specified.

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
```

The folder pointed to by `GST_PKG_CONFIG` should have the following structure:

* pkgconfig
  * armv7
  * arm64
  * x86
  * x86\_64

To build the rust library:

```
cd examples/sink
./gradlew cargoBuild
```

In order to build the whole project, all the gstreamer libraries used should be copied into examples/sink/app/build/rustJniLibs.

To build and install the android package:

```
cd examples/sink
./gradlew installDebug
```
