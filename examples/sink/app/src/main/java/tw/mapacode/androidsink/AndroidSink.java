package tw.mapacode.androidsink;

import android.content.Context;
import android.widget.Toast;

import org.freedesktop.gstreamer.GStreamer;

public class AndroidSink {
    static {
        System.loadLibrary("androidsink");
    }

    private static native void nativeRun();
    private static AndroidSink INSTANCE = null;

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
