# CMakeLists.txt

cmake_minimum_required(VERSION 3.10)
project(androidsink VERSION 0.1.0)

if (NOT DEFINED ENV{GST_PKG_CONFIG})
    message("Please set environment variable GST_PKG_CONFIG")
elseif (ANDROID_ABI STREQUAL "x86")
    add_custom_target(x86 ALL
        COMMENT "Building x86 version of androidsink library"
        COMMAND PKG_CONFIG_LIBDIR=$ENV{GST_PKG_CONFIG}/x86 cargo ndk --platform=21 --target=i686-linux-android build --release
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
elseif (ANDROID_ABI STREQUAL "x86_64")
    add_custom_target(x86_64 ALL
        COMMENT "Building x86_64 version of androidsink library"
        COMMAND PKG_CONFIG_LIBDIR=$ENV{GST_PKG_CONFIG}/x86_64 cargo ndk --platform=21 --target=x86_64-linux-android build --release
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
elseif (ANDROID_ABI STREQUAL "armeabi-v7a")
    add_custom_target(armeabi-v7a ALL
        COMMENT "Building arm version of androidsink library"
        COMMAND PKG_CONFIG_LIBDIR=$ENV{GST_PKG_CONFIG}/armv7 cargo ndk --platform=21 --target=armv7-linux-androideabi build --release
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
elseif (ANDROID_ABI STREQUAL "arm64-v8a")
    add_custom_target(arm64-v8a ALL
        COMMENT "Building aarch64 version of androidsink library"
        COMMAND PKG_CONFIG_LIBDIR=$ENV{GST_PKG_CONFIG}/arm64 cargo ndk --platform=21 --target=aarch64-linux-android build --release
        WORKING_DIRECTORY ${CMAKE_CURRENT_SOURCE_DIR})
else()
    message ("found no ABI")
endif()