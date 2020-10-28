use dlopen::symbor::Library;
use jni::objects::{GlobalRef, JClass, JObject, JValue};
use jni::sys::jint;
use jni::{JNIEnv, JavaVM};
use libc::{c_int, c_void, pthread_self};
use std::ffi::CString;
use std::fmt::Write;
use std::path::Path;

use glib::{ObjectExt, ObjectType};
use gst::util_get_timestamp;
use gst::{ClockTime, DebugCategory, DebugLevel, DebugMessage, Pad};
use gst_sys;

use ndk_sys::android_LogPriority_ANDROID_LOG_DEBUG as ANDROID_LOG_DEBUG;
use ndk_sys::android_LogPriority_ANDROID_LOG_ERROR as ANDROID_LOG_ERROR;
use ndk_sys::android_LogPriority_ANDROID_LOG_INFO as ANDROID_LOG_INFO;
use ndk_sys::android_LogPriority_ANDROID_LOG_VERBOSE as ANDROID_LOG_VERBOSE;
use ndk_sys::android_LogPriority_ANDROID_LOG_WARN as ANDROID_LOG_WARN;

static mut JAVA_VM: Option<JavaVM> = None;
static mut PLUGIN_NAMES: Vec<&'static str> = Vec::new();
static mut PLUGINS: Vec<Library> = Vec::new();
static mut GST_INFO_START_TIME: ClockTime = ClockTime(None);
static mut CONTEXT: Option<GlobalRef> = None;
static mut CLASS_LOADER: Option<GlobalRef> = None;
static mut GST_DEBUG_LOG_FUNCTION: Option<gst::DebugLogFunction> = None;

macro_rules! gstinit_trace {
    ($($arg:tt)*) => {
        let mut msg = String::new();
        msg.write_fmt(format_args!($($arg)*)).unwrap();
        android_log_write(ANDROID_LOG_VERBOSE as c_int, CString::new("GStreamer+androidinit").unwrap(), CString::new(msg).unwrap());
    }
}

macro_rules! gstinit_error {
    ($($arg:tt)*) => {
        let mut msg = String::new();
        msg.write_fmt(format_args!($($arg)*)).unwrap();
        android_log_write(ANDROID_LOG_ERROR as c_int, CString::new("GStreamer+androidinit").unwrap(), CString::new(msg).unwrap());
    }
}

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

fn glib_print_handler(msg: &str) {
    android_log_write(
        ANDROID_LOG_INFO as c_int,
        CString::new("GLib+stdout").unwrap(),
        CString::new(msg).unwrap(),
    );
}

fn glib_printerr_handler(msg: &str) {
    android_log_write(
        ANDROID_LOG_ERROR as c_int,
        CString::new("GLib+stderr").unwrap(),
        CString::new(msg).unwrap(),
    );
}

fn glib_log_handler(domain: &str, level: glib::LogLevel, msg: &str) {
    let prio = match level {
        glib::LogLevel::Error => ANDROID_LOG_ERROR,
        glib::LogLevel::Critical => ANDROID_LOG_ERROR,
        glib::LogLevel::Warning => ANDROID_LOG_WARN,
        glib::LogLevel::Message => ANDROID_LOG_INFO,
        glib::LogLevel::Info => ANDROID_LOG_INFO,
        glib::LogLevel::Debug => ANDROID_LOG_DEBUG,
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
        DebugLevel::Error => ANDROID_LOG_ERROR,
        DebugLevel::Warning => ANDROID_LOG_WARN,
        DebugLevel::Info => ANDROID_LOG_INFO,
        DebugLevel::Debug => ANDROID_LOG_DEBUG,
        _ => ANDROID_LOG_VERBOSE,
    };

    let tag = CString::new(String::from("GStreamer+") + category.get_name()).unwrap();
    let mut label = String::new();
    match object {
        Some(obj) => {
            if obj.is::<Pad>() {
                let pad_name: Option<&str> = unsafe {
                    let ptr = obj.as_ptr() as *mut gst_sys::GstObject;
                    if (*ptr).name.is_null() {
                        None
                    } else {
                        std::ffi::CStr::from_ptr((*ptr).name).to_str().ok()
                    }
                };

                let parent_name: Option<&str> = unsafe {
                    let ptr = obj.as_ptr() as *mut gst_sys::GstObject;
                    if (*ptr).parent.is_null() {
                        None
                    } else {
                        let ptr = (*ptr).parent;
                        if (*ptr).name.is_null() {
                            None
                        } else {
                            std::ffi::CStr::from_ptr((*ptr).name).to_str().ok()
                        }
                    }
                };
                write!(
                    &mut label,
                    "<{}:{}>",
                    parent_name.unwrap_or(""),
                    pad_name.unwrap_or("")
                )
                .unwrap();
            } else if obj.is::<gst::Object>() {
                let name: Option<&str> = unsafe {
                    let ptr = obj.as_ptr() as *mut gst_sys::GstObject;
                    if (*ptr).name.is_null() {
                        None
                    } else {
                        std::ffi::CStr::from_ptr((*ptr).name).to_str().ok()
                    }
                };
                write!(&mut label, "<{}>", name.unwrap_or("")).unwrap();
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

// Get application's cache directory and files directory.
fn get_application_dirs(env: &JNIEnv, context: JObject) -> (String, String) {
    let cache_dir_path_str;
    if let JValue::Object(cache_dir) = env
        .call_method(context, "getCacheDir", "()Ljava/io/File;", &[])
        .expect("Could not call getCacheDir")
    {
        if let JValue::Object(cache_dir_path) = env
            .call_method(cache_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .expect("Could not call getAbsolutePath")
        {
            cache_dir_path_str = env
                .get_string(cache_dir_path.into())
                .expect("Could not get string for cached dir path");
        } else {
            unreachable!();
        }
    } else {
        unreachable!();
    }

    let files_dir_path_str;
    if let JValue::Object(files_dir) = env
        .call_method(context, "getFilesDir", "()Ljava/io/File;", &[])
        .expect("Could not call getFilesDir")
    {
        if let JValue::Object(files_dir_path) = env
            .call_method(files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
            .expect("Could not call getAbsolutePath")
        {
            files_dir_path_str = env
                .get_string(files_dir_path.into())
                .expect("Could not get string from files dir path");
        } else {
            unreachable!();
        }
    } else {
        unreachable!();
    }

    (cache_dir_path_str.into(), files_dir_path_str.into())
}

fn gst_android_load_gio_modules() {
    // TODO
}

#[no_mangle]
pub unsafe extern "C" fn Java_org_freedesktop_gstreamer_GStreamer_nativeInit(
    env: JNIEnv,
    _: JClass,
    context: JObject,
) {
    gstinit_trace!("GStreamer.init()");

    // Store context and class cloader.
    match env.call_method(context, "getClassLoader", "()Ljava/lang/ClassLoader;", &[]) {
        Ok(loader) => {
            match loader {
                JValue::Object(obj) => {
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
                            gstinit_error!("{}", e);
                            return;
                        }
                    }
                }
                _ => {
                    gstinit_error!("Could not get class loader");
                    return;
                }
            }
        }
        Err(e) => {
            gstinit_error!("{}", e);
            return;
        }
    }

    if gst_sys::gst_is_initialized() == glib_sys::GTRUE {
        gstinit_error!("GStreamer already initialized");
        return;
    }

    let (cache_dir, files_dir) = get_application_dirs(&env, context);
    gstinit_trace!("cache_dir: {}, files_dir: {}", cache_dir, files_dir);
    // Set environment variables to cache dir, on some platforms not thread-safe beacause of `std::env.set_var`.
    std::env::set_var("TMP", &cache_dir);
    std::env::set_var("TMP", &cache_dir);
    std::env::set_var("TEMP", &cache_dir);
    std::env::set_var("TMPDIR", &cache_dir);
    std::env::set_var("XDG_RUNTIME_DIR", &cache_dir);
    std::env::set_var("XDG_CACHE_HOME", &cache_dir);
    let registry = Path::new(&cache_dir).join("registry.bin");
    gstinit_trace!("registry: {:?}", &registry);
    std::env::set_var("GST_REGISTRY", &registry);
    std::env::set_var("GST_REGISTRY_REUSE_PLUGIN_SCANNER", "no");
    // Set std::environment variables to files dir, on some platforms not thread-safe beacause of `std::env::set_var`.
    std::env::set_var("HOME", &files_dir);
    std::env::set_var("XDG_DATA_DIRS", &files_dir);
    std::env::set_var("XDG_CONFIG_DIRS", &files_dir);
    std::env::set_var("XDG_CONFIG_HOME", &files_dir);
    std::env::set_var("XDG_DATA_HOME", &files_dir);
    let fontconfig = Path::new(&files_dir).join("fontconfig");
    gstinit_trace!("fontconfig path: {:?}", &fontconfig);
    std::env::set_var("FONTCONFIG_PATH", &fontconfig);
    let certs = Path::new(&files_dir)
        .join("ssl")
        .join("certs")
        .join("ca-certificates.crt");
    gstinit_trace!("ca_certificates path: {:?}", &certs);
    std::env::set_var("CA_CERTIFICATES", &certs);

    // Set GLIB print handlers
    gstinit_trace!("set glib handlers");
    glib::set_print_handler(glib_print_handler);
    glib::set_printerr_handler(glib_printerr_handler);
    glib::log_set_default_handler(glib_log_handler);

    // Disable this for releases if performance is important
    // or increase the threshold to get more information
    gst::debug_set_active(true);
    gst::debug_set_default_threshold(gst::DebugLevel::Warning);
    gst::debug_remove_default_log_function();
    GST_DEBUG_LOG_FUNCTION = Some(gst::debug_add_log_function(debug_logcat));

    GST_INFO_START_TIME = util_get_timestamp();

    gstinit_trace!("gst init");
    match gst::init() {
        Ok(_) => { /* Do nothing. */ }
        Err(e) => {
            gstinit_error!("GStreamer initialization failed: {}", e);
            let mut msg = String::new();
            write!(msg, "GStreamer initialization failed: {}", e).unwrap();
            match env.find_class("java/lang/Exception") {
                Ok(c) => {
                    let _ = env.throw_new(c, &msg);
                }
                Err(e) => {
                    gstinit_error!("Could not get Exception class: {}", e);
                    return;
                }
            }
            return;
        }
    }

    {
        gstinit_trace!("load plugins");
        for name in &PLUGIN_NAMES {
            let mut so_name = String::from("libgst");
            so_name.push_str(name);
            so_name.push_str(".so");
            gstinit_trace!("loading {}", so_name);
            match Library::open(&so_name) {
                Ok(lib) => {
                    // Register plugin
                    let mut plugin_register = String::from("gst_plugin_");
                    plugin_register.push_str(name);
                    plugin_register.push_str("_register");
                    gstinit_trace!("registering {}", so_name);
                    match lib.symbol::<unsafe extern "C" fn()>(&plugin_register) {
                        Ok(f) => f(),
                        Err(e) => {
                            gstinit_error!("{}", e);
                        }
                    }
                    // Keep plugin
                    PLUGINS.push(lib);
                }
                Err(e) => {
                    gstinit_error!("{}", e);
                }
            };
        }
    }

    gst_android_load_gio_modules();
}

pub unsafe fn on_load(
    jvm: JavaVM,
    _reserved: *mut c_void,
    plugin_names: Vec<&'static str>,
) -> jint {
    PLUGIN_NAMES = plugin_names;

    gstinit_trace!("get JNIEnv");

    let env: JNIEnv;
    match jvm.get_env() {
        Ok(v) => {
            env = v;
        }
        Err(e) => {
            gstinit_error!("Could not retrieve JNIEnv, error: {}", e);
            return 0;
        }
    }

    // TODO: check the version > JNI_VERSION_1_4

    gstinit_trace!("get JNI version");

    let version: jint;
    match env.get_version() {
        Ok(v) => {
            version = v.into();
            gstinit_trace!("JNI Version: {:#x?}", version);
        }
        Err(e) => {
            gstinit_error!("Could not retrieve JNI version, error: {}", e);
            return 0;
        }
    }

    gstinit_trace!("find class GStreamer");

    match env.find_class("org/freedesktop/gstreamer/GStreamer") {
        Ok(_c) => {}
        Err(e) => {
            gstinit_error!(
                "Could not retreive class org.freedesktop.gstreamer.GStreamer, error: {}",
                e
            );
            return 0;
        }
    }

    gstinit_trace!("save java vm");

    /* Remember Java VM */
    JAVA_VM = Some(jvm);

    version
}
