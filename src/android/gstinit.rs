use dlopen::symbor::Library;
use jni::objects::{GlobalRef, JClass, JObject};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};
use libc::{c_int, c_void, pthread_self};
use std::ffi::CString;
use std::fmt::Write;

use glib::translate::*;
use glib::{Cast, GString, ObjectExt};
use gst::util_get_timestamp;
use gst::{ClockTime, DebugCategory, DebugLevel, DebugMessage, GstObjectExt, Pad};
use gst_sys;

static mut JAVA_VM: Option<JavaVM> = None;
static mut PLUGINS: Vec<Library> = Vec::new();
static mut GST_INFO_START_TIME: ClockTime = ClockTime(None);
static mut CONTEXT: Option<GlobalRef> = None;
static mut CLASS_LOADER: Option<GlobalRef> = None;
static mut GST_DEBUG_LOG_FUNCTION: Option<gst::DebugLogFunction> = None;

use once_cell::sync::Lazy;
pub static CAT: Lazy<gst::DebugCategory> = Lazy::new(|| {
    gst::DebugCategory::new(
        "gstinit",
        gst::DebugColorFlags::empty(),
        Some("GStreamer+init"),
    )
});

fn android_log_write(prio: c_int, tag: CString, msg: CString) {
    unsafe {
        ndk_sys::__android_log_write(prio, tag.as_ptr(), msg.as_ptr());
    }
}

fn glib_print_info(msg: &str) {
    android_log_write(
        ndk_sys::android_LogPriority_ANDROID_LOG_INFO as c_int,
        CString::new("GLib+stdout").unwrap(),
        CString::new(msg).unwrap(),
    );
}

fn glib_print_error(msg: &str) {
    android_log_write(
        ndk_sys::android_LogPriority_ANDROID_LOG_ERROR as c_int,
        CString::new("GLib+stderr").unwrap(),
        CString::new(msg).unwrap(),
    );
}

fn glib_log_with_domain(domain: &str, level: glib::LogLevel, msg: &str) {
    let prio = match level {
        glib::LogLevel::Error => ndk_sys::android_LogPriority_ANDROID_LOG_ERROR,
        glib::LogLevel::Critical => ndk_sys::android_LogPriority_ANDROID_LOG_ERROR,
        glib::LogLevel::Warning => ndk_sys::android_LogPriority_ANDROID_LOG_WARN,
        glib::LogLevel::Message => ndk_sys::android_LogPriority_ANDROID_LOG_INFO,
        glib::LogLevel::Info => ndk_sys::android_LogPriority_ANDROID_LOG_INFO,
        glib::LogLevel::Debug => ndk_sys::android_LogPriority_ANDROID_LOG_DEBUG,
    };
    let tag = CString::new(String::from("Glib+") + domain).unwrap();
    android_log_write(prio as c_int, tag, CString::new(msg).unwrap());
}

fn debug_logcat(
    category: DebugCategory,
    level: DebugLevel,
    file: &str,
    function: &str,
    line: u32,
    object: Option<&glib::object::Object>,
    message: &DebugMessage,
) {
    if level > category.get_threshold() {
        return;
    }

    let elapsed = util_get_timestamp() - unsafe { GST_INFO_START_TIME };

    let lvl = match level {
        DebugLevel::Error => ndk_sys::android_LogPriority_ANDROID_LOG_ERROR,
        DebugLevel::Warning => ndk_sys::android_LogPriority_ANDROID_LOG_WARN,
        DebugLevel::Info => ndk_sys::android_LogPriority_ANDROID_LOG_INFO,
        DebugLevel::Debug => ndk_sys::android_LogPriority_ANDROID_LOG_DEBUG,
        _ => ndk_sys::android_LogPriority_ANDROID_LOG_VERBOSE,
    };

    let tag = CString::new(String::from("GStreamer+") + category.get_name()).unwrap();
    let mut label = String::new();
    match object {
        Some(obj) => {
            if obj.is::<Pad>() {
                let pad = obj.downcast_ref::<gst::Object>().unwrap();
                // Do not use pad.get_name() here because get_name() may fail because the object may not yet fully constructed.
                let pad_name: Option<GString> = unsafe {
                    from_glib_full(gst_sys::gst_object_get_name(pad.to_glib_none().0))
                };
                let pad_name = pad_name.map_or("".to_string(), |v| v.to_string());
                let parent_name = pad
                    .get_parent()
                    .map_or("".to_string(), |p| p.get_name().to_string());
                write!(&mut label, "<{}:{}>", parent_name, pad_name).unwrap();
            } else if obj.is::<gst::Object>() {
                let ob = obj.downcast_ref::<gst::Object>().unwrap();
                // Do not use ob.get_name() here because get_name() may fail because the object may not yet fully constructed.
                let ob_name: Option<GString> = unsafe {
                    from_glib_full(gst_sys::gst_object_get_name(ob.to_glib_none().0))
                };
                let name = ob_name.map_or("".to_string(), |v| v.to_string());
                write!(&mut label, "<{}>", name).unwrap();
            } else {
                write!(&mut label, "<{}@{:#x?}>", obj.get_type(), obj).unwrap();
            }
            let mut msg = String::with_capacity(128);
            write!(
                msg,
                "{} {:#x?} {}:{}:{}:{} {}",
                elapsed,
                unsafe { pthread_self() },
                file,
                line,
                function,
                label,
                message.get().unwrap()
            )
            .unwrap();
            android_log_write(lvl as c_int, tag, CString::new(msg).unwrap());
        }
        None => {
            let mut msg = String::with_capacity(128);
            write!(
                msg,
                "{} {:#x?} {}:{}:{} {}",
                elapsed,
                unsafe { pthread_self() },
                file,
                line,
                function,
                message.get().unwrap()
            )
            .unwrap();
            android_log_write(lvl as c_int, tag, CString::new(msg).unwrap());
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gst_android_get_java_vm() -> *const jni::sys::JavaVM {
    match &JAVA_VM {
        Some(vm) => vm.get_java_vm_pointer(),
        None => {
            gst_trace!(CAT, "Could not get jvm");
            return std::ptr::null();
        }
    }
}

#[no_mangle]
pub unsafe extern "C" fn gst_android_get_application_context() -> jni::sys::jobject {
    match &CONTEXT {
        Some(c) => c.as_obj().into_inner(),
        None => std::ptr::null_mut(),
    }
}

#[no_mangle]
pub unsafe extern "C" fn gst_android_get_application_class_loader() -> jni::sys::jobject {
    match &CLASS_LOADER {
        Some(o) => o.as_obj().into_inner(),
        None => std::ptr::null_mut(),
    }
}


#[no_mangle]
pub unsafe extern "C" fn Java_org_freedesktop_gstreamer_GStreamer_nativeInit(
    env: JNIEnv,
    _: JClass,
    context: JObject,
) {
    gst_trace!(CAT, "GStreamer.init()");

    // Store context and class cloader.
    match env.call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[]) {
        Ok(loader) => {
            match loader {
                jni::objects::JValue::Object(obj) => {
                    CONTEXT = env.new_global_ref(context).ok();
                    CLASS_LOADER = env.new_global_ref(obj).ok();
                    match env.exception_check() {
                        Ok(value) => {
                            if value {
                                env.exception_describe().unwrap();
                                env.exception_clear().unwrap();
                                return;
                            } else {
                                // Do nothing.
                            }
                        }
                        Err(e) => {
                            gst_trace!(CAT, "{}", e);
                            return;
                        }
                    }
                }
                _ => {
                    gst_trace!(CAT, "Could not get class loader");
                    return;
                }
            }
        }
        Err(e) => {
            gst_trace!(CAT, "{}", e);
            return;
        }
    }

    // Set GLIB print handlers
    gst_trace!(CAT, "set glib handlers");
    glib::set_print_handler(glib_print_info);
    glib::set_printerr_handler(glib_print_error);
    glib::log_set_default_handler(glib_log_with_domain);

    // Disable this for releases if performance is important
    // or increase the threshold to get more information
    gst::debug_set_active(true);
    gst::debug_set_default_threshold(gst::DebugLevel::Trace);
    gst::debug_remove_default_log_function();
    GST_DEBUG_LOG_FUNCTION = Some(gst::debug_add_log_function(debug_logcat));

    GST_INFO_START_TIME = util_get_timestamp();

    gst_trace!(CAT, "gst init");
    match gst::init() {
        Ok(_) => { /* Do nothing. */ }
        Err(e) => {
            gst_trace!(CAT, "{}", e);
            return;
        }
    }

    {
        gst_trace!(CAT, "load plugins");
        let mut plugins_core = vec![
            "coreelements",
            "coretracers",
            "adder",
            "app",
            "audioconvert",
            "audiomixer",
            "audiorate",
            "audioresample",
            "audiotestsrc",
            "compositor",
            "gio",
            "overlaycomposition",
            "pango",
            "rawparse",
            "typefindfunctions",
            "videoconvert",
            "videorate",
            "videoscale",
            "videotestsrc",
            "volume",
            "autodetect",
            "videofilter",
        ];
        let mut plugins_codecs = vec!["androidmedia"];
        let mut plugins = Vec::new();
        plugins.append(&mut plugins_core);
        plugins.append(&mut plugins_codecs);

        for name in &plugins {
            let mut so_name = String::from("libgst");
            so_name.push_str(name);
            so_name.push_str(".so");
            gst_trace!(CAT, "loading {}", so_name);
            match Library::open(&so_name) {
                Ok(lib) => {
                    // Register plugin
                    let mut plugin_register = String::from("gst_plugin_");
                    plugin_register.push_str(name);
                    plugin_register.push_str("_register");
                    gst_trace!(CAT, "registering {}", so_name);
                    match lib.symbol::<unsafe extern "C" fn()>(&plugin_register) {
                        Ok(f) => f(),
                        Err(e) => {
                            gst_trace!(CAT, "{}", e);
                        }
                    }
                    // Keep plugin
                    PLUGINS.push(lib);
                }
                Err(e) => {
                    gst_trace!(CAT, "{}", e);
                }
            };
        }
    }
}

pub unsafe fn on_load(jvm: JavaVM, _reserved: *mut c_void) -> jint {
    gst_trace!(CAT, "get JNIEnv");

    let env: JNIEnv;
    match jvm.get_env() {
        Ok(v) => {
            env = v;
        }
        Err(e) => {
            gst_trace!(CAT, "Could not retrieve JNIEnv, error: {}", e);
            return 0;
        }
    }

    // TODO: check the version > JNI_VERSION_1_4

    gst_trace!(CAT, "get JNI version");

    let version: jint;
    match env.get_version() {
        Ok(v) => {
            version = v.into();
            gst_trace!(CAT, "JNI Version: {:#x?}", version);
        }
        Err(e) => {
            gst_trace!(CAT, "Could not retrieve JNI version, error: {}", e);
            return 0;
        }
    }

    gst_trace!(CAT, "find class GStreamer");

    match env.find_class("org/freedesktop/gstreamer/GStreamer") {
        Ok(_c) => {}
        Err(e) => {
            gst_trace!(
                CAT,
                "Could not retreive class org.freedesktop.gstreamer.GStreamer, error: {}",
                e
            );
            return 0;
        }
    }

    gst_trace!(CAT, "save java vm");

    /* Remember Java VM */
    JAVA_VM = Some(jvm);

    version
}