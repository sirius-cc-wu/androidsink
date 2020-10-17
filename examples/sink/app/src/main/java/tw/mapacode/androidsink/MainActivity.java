package tw.mapacode.androidsink;

import androidx.appcompat.app.AppCompatActivity;

import android.widget.Toast;
import android.os.Bundle;

public class MainActivity extends AppCompatActivity {

    private static boolean gst_initialized = false;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        setContentView(R.layout.activity_main);

        AndroidSink sink = AndroidSink.getInstance(this, 0,0);
        if (sink != null) {
            sink.start();
        }
    }
}