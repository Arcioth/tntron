package com.arcioth.tntron.android;

import android.os.Handler;
import android.os.Looper;

import org.json.JSONObject;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.function.Consumer;

public final class TntronRemoteClient {
    public interface Listener {
        void onConnectionChanged(boolean connected, String message);
        void onStatus(JSONObject status);
    }

    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService cmdExecutor = Executors.newSingleThreadExecutor();
    private final Listener listener;

    private volatile String host = "127.0.0.1";
    private volatile int port = 26263;
    private volatile String authToken = "";

    private volatile boolean running = false;
    private volatile boolean connected = false;
    private volatile Socket subSocket;
    private Socket cmdSocket;
    private BufferedReader cmdReader;
    private BufferedWriter cmdWriter;
    private final Object cmdLock = new Object();
    private Thread subThread;

    public TntronRemoteClient(Listener listener) {
        this.listener = listener;
    }

    public boolean isConnected() {
        return connected;
    }

    public void configureAndConnect(String host, int port, String authToken) {
        this.host = (host == null || host.trim().isEmpty()) ? "127.0.0.1" : host.trim();
        this.port = (port < 1024 || port > 65535) ? 26263 : port;
        this.authToken = authToken == null ? "" : authToken.trim();
        stopSocketsOnly();
        if (!running) {
            running = true;
            subThread = new Thread(this::subscriberLoop, "tntron-android-sub");
            subThread.setDaemon(true);
            subThread.start();
        }
    }

    public void shutdown() {
        running = false;
        stopSocketsOnly();
        cmdExecutor.shutdownNow();
    }

    private void stopSocketsOnly() {
        connected = false;
        closeQuiet(subSocket);
        subSocket = null;
        synchronized (cmdLock) {
            closeCmdLocked();
        }
    }

    private void closeCmdLocked() {
        closeQuiet(cmdSocket);
        cmdSocket = null;
        cmdReader = null;
        cmdWriter = null;
    }

    private static void closeQuiet(Socket s) {
        if (s != null) {
            try { s.close(); } catch (IOException ignored) {}
        }
    }

    private Socket openSocket() throws IOException {
        Socket s = new Socket();
        s.connect(new InetSocketAddress(host, port), 2500);
        s.setTcpNoDelay(true);
        return s;
    }

    private void authIfNeeded(BufferedWriter writer, BufferedReader reader) throws Exception {
        if (!authToken.isEmpty()) {
            JSONObject auth = new JSONObject();
            auth.put("cmd", "auth");
            auth.put("token", authToken);
            writer.write(auth.toString());
            writer.write('\n');
            writer.flush();
            String respLine = reader.readLine();
            if (respLine == null) throw new EOFException("Closed during auth");
            JSONObject resp = new JSONObject(respLine);
            if (!resp.optBoolean("ok", false)) {
                throw new IOException(resp.optString("error", "Authentication failed"));
            }
        }
    }

    private void subscriberLoop() {
        while (running) {
            try {
                notifyConnection(false, "Connecting to " + host + ":" + port + "...");
                Socket s = openSocket();
                subSocket = s;
                BufferedReader reader = new BufferedReader(new InputStreamReader(s.getInputStream(), StandardCharsets.UTF_8));
                BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(s.getOutputStream(), StandardCharsets.UTF_8));

                authIfNeeded(writer, reader);

                writer.write("{\"cmd\":\"subscribe\"}\n");
                writer.flush();

                connected = true;
                notifyConnection(true, "Connected (" + host + ":" + port + ")");

                String line;
                while (running && (line = reader.readLine()) != null) {
                    if (line.trim().isEmpty()) continue;
                    try {
                        JSONObject st = new JSONObject(line);
                        mainHandler.post(() -> listener.onStatus(st));
                    } catch (Exception ignored) {
                    }
                }
            } catch (Exception e) {
                connected = false;
                String msg = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
                notifyConnection(false, "Offline: " + msg);
            } finally {
                connected = false;
                closeQuiet(subSocket);
                subSocket = null;
            }

            if (running) {
                try {
                    Thread.sleep(2000);
                } catch (InterruptedException ignored) {
                    break;
                }
            }
        }
    }

    private void notifyConnection(boolean isConn, String msg) {
        mainHandler.post(() -> listener.onConnectionChanged(isConn, msg));
    }

    private JSONObject sendSync(JSONObject payload) {
        synchronized (cmdLock) {
            for (int attempt = 0; attempt < 2; attempt++) {
                try {
                    if (cmdSocket == null || cmdSocket.isClosed() || !cmdSocket.isConnected()) {
                        cmdSocket = openSocket();
                        cmdSocket.setSoTimeout(3000);
                        cmdReader = new BufferedReader(new InputStreamReader(cmdSocket.getInputStream(), StandardCharsets.UTF_8));
                        cmdWriter = new BufferedWriter(new OutputStreamWriter(cmdSocket.getOutputStream(), StandardCharsets.UTF_8));
                        authIfNeeded(cmdWriter, cmdReader);
                    }
                    cmdWriter.write(payload.toString());
                    cmdWriter.write('\n');
                    cmdWriter.flush();
                    String respLine = cmdReader.readLine();
                    if (respLine == null) throw new EOFException("EOF");
                    return new JSONObject(respLine);
                } catch (Exception ignored) {
                    closeCmdLocked();
                }
            }
            return null;
        }
    }

    public void send(JSONObject payload, Consumer<JSONObject> onDone) {
        if (cmdExecutor.isShutdown()) return;
        cmdExecutor.submit(() -> {
            JSONObject resp = sendSync(payload);
            if (onDone != null) {
                mainHandler.post(() -> onDone.accept(resp));
            }
        });
    }

    public void sendCmd(String cmd) {
        try {
            JSONObject o = new JSONObject();
            o.put("cmd", cmd);
            send(o, null);
        } catch (Exception ignored) {}
    }

    public void sendWith(String cmd, Consumer<JSONObject> builder, Consumer<JSONObject> onDone) {
        try {
            JSONObject o = new JSONObject();
            o.put("cmd", cmd);
            if (builder != null) builder.accept(o);
            send(o, onDone);
        } catch (Exception ignored) {}
    }
}
