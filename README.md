# Androidsink

Android example for gstreamer-rs

# Build

```
# Environment variables
export ANDROID_HOME=<path/to/android/home>
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/tools/bin:$ANDROID_HOME/build-tools/29.0.2:$ANDROID_HOME/platform-tools
export ANDROID_NDK_HOME=<path/to/ndk>
export PATH=$PATH:$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang
export CXX=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang++
export PKG_CONFIG_ALLOW_CROSS=1 

# Rust library
export GST_PKG_CONFIG=<path/to/gstreamer/pkgconfig>
PKG_CONFIG_LIBDIR=$GST_PKG_CONFIG/x86 cargo ndk --platform=21 --target=i686-linux-android build --release
PKG_CONFIG_LIBDIR=$GST_PKG_CONFIG/x86_64 cargo ndk --platform=21 --target=x86_64-linux-android build --release
PKG_CONFIG_LIBDIR=$GST_PKG_CONFIG/armv7 cargo ndk --platform=21 --target=armv7-linux-androideabi build --release
PKG_CONFIG_LIBDIR=$GST_PKG_CONFIG/arm64 cargo ndk --platform=21 --target=aarch64-linux-android build --release
mkdir -p examples/sink/app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86,x86_64}
cp ./target/armv7-linux-androideabi/release/libandroidsink.so examples/sink/app/src/main/jniLibs/armeabi-v7a/
cp ./target/aarch64-linux-android/release/libandroidsink.so examples/sink/app/src/main/jniLibs/arm64-v8a/
cp ./target/x86_64-linux-android/release/libandroidsink.so examples/sink/app/src/main/jniLibs/x86_64/
cp ./target/i686-linux-android/release/libandroidsink.so examples/sink/app/src/main/jniLibs/x86/

# Android
cd examples/sink
./gradlew build
./gradlew installDebug
```

# 文獻

* [Create a Basic Android App without an IDE ](https://developer.okta.com/blog/2018/08/10/basic-android-without-an-ide)
