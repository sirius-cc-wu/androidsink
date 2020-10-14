package tw.mapacode.androidsink;

public class AndroidSink {
    static {
        System.loadLibrary("androidsink");
    }

    private static native void nativeRun();

    private static AndroidSink INSTANCE = null;

    private AndroidSink() {};

    public static AndroidSink getInstance(int sampleRate, int bufSize) {
        if (INSTANCE == null) {
//            if (sampleRate != 0) {
//                nativeSetSampleRate(sampleRate);
//            }
//            if (bufSize != 0) {
//                nativeSetBufSize(bufSize);
//            }
            INSTANCE = new AndroidSink();
        }
        return(INSTANCE);
    }

    public void start() {
        nativeRun();
    }

}
