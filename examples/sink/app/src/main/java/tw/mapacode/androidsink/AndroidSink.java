package tw.mapacode.androidsink;

import android.content.Context;
import android.widget.Toast;
import android.system.Os;
import android.util.Log;

import org.freedesktop.gstreamer.GStreamer;

public class AndroidSink {
    static {
        System.loadLibrary("z");
        System.loadLibrary("androidsink");
    }

    private static native void nativeRun();
    private static AndroidSink INSTANCE = null;
    private static final String tag = "Androidsink";

    private AndroidSink() {};

    public static AndroidSink getInstance(Context context, int sampleRate, int bufSize) {
        if (INSTANCE == null) {
//            if (sampleRate != 0) {
//                nativeSetSampleRate(sampleRate);
//            }
//            if (bufSize != 0) {
//                nativeSetBufSize(bufSize);
//            }
            // Initialize GStreamer and warn if it fails
            try {
                // Os.setenv("GST_DEBUG", "androidsink:7", true);
                // Os.setenv("GST_DEBUG", "androidsink:7,basesrc:6,basesink:6,fakesrc:6,fakesink:6", true);
                // Os.setenv("GST_DEBUG", "GST_ELEMENT_FACTORY:7", true);
                // Os.setenv("GST_DEBUG", "androidsink:6,basesrc:6,basesink:6", true);
                // Os.setenv("GST_DEBUG", "androidsink:6,basesrc:6,basesink:6,audiotestsrc:6,appsink:6", true);
                // Os.setenv("GST_DEBUG", "GST_ELEMENT_PADS:6", true);
                Os.setenv("GST_DEBUG", "GST_DEBUG:6", true);
                // Os.setenv("GST_DEBUG", "6", true);
            } catch (Exception e) {
                Log.i(tag,"Cannot set environment variables");
            }

            try {
                GStreamer.init(context);
            } catch (Exception e) {
                Toast.makeText(context, e.getMessage(), Toast.LENGTH_LONG).show();
                return null;
            }
            INSTANCE = new AndroidSink();
        }
        return(INSTANCE);
    }

    public void start() {
        nativeRun();
    }

}
