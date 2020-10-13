# Androidsink

Android example for gstreamer-rs

# Build

```
# Environment variables
export ANDROID_HOME=<path/to/android/home>
export PATH=$PATH:$ANDROID_HOME/cmdline-tools/tools/bin:$ANDROID_HOME/build-tools/29.0.2:$ANDROID_HOME/platform-tools
export NDK_HOME=<path/to/ndk>
export PATH=$PATH:$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin
export CC=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang
export CXX=$NDK_HOME/toolchains/llvm/prebuilt/linux-x86_64/bin/clang++

# Rust library
cargo ndk --platform=26 --target=i686-linux-android build --release
cargo ndk --platform=26 --target=x86_64-linux-android build --release
cargo ndk --platform=26 --target=armv7-linux-androideabi build --release
cargo ndk --platform=26 --target=aarch64-linux-android build --release
cd examples/sink/app/src/main
mkdir -p examples/sink/app/src/main/jniLibs/{arm64-v8a,armeabi-v7a,x86,x86_64}
cp ./target/armv7-linux-androideabi/release/libmic.so examples/sink/app/src/main/jniLibs/armeabi-v7a/
cp ./target/aarch64-linux-android/release/libmic.so examples/sink/app/src/main/jniLibs/arm64-v8a/
cp ./target/x86_64-linux-android/release/libmic.so examples/sink/app/src/main/jniLibs/x86_64/
cp ./target/i686-linux-android/release/libmic.so examples/sink/app/src/main/jniLibs/x86/

# Android
cd examples/sink
./gradlew build
./gradlew installDebug
```

# 文獻

* [Create a Basic Android App without an IDE ](https://developer.okta.com/blog/2018/08/10/basic-android-without-an-ide)
