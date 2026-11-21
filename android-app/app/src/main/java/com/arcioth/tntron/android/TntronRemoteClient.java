package com.arcioth.tntron.android;

import android.annotation.SuppressLint;
import android.bluetooth.BluetoothAdapter;
import android.bluetooth.BluetoothDevice;
import android.bluetooth.BluetoothSocket;
import android.os.Handler;
import android.os.Looper;

import org.json.JSONObject;

import java.io.BufferedReader;
import java.io.BufferedWriter;
import java.io.EOFException;
import java.io.IOException;
import java.io.InputStreamReader;
import java.io.OutputStreamWriter;
import java.lang.reflect.Method;
import java.net.InetSocketAddress;
import java.net.Socket;
import java.nio.charset.StandardCharsets;
import java.util.UUID;
import java.util.concurrent.ConcurrentLinkedQueue;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;
import java.util.function.Consumer;

public final class TntronRemoteClient {
    public static final int BT_RFCOMM_CHANNEL = 22;
    private static final UUID SPP_UUID = UUID.fromString("00001101-0000-1000-8000-00805F9B34FB");

    public enum Mode { BLUETOOTH, TCP }

    public interface Listener {
        void onConnectionChanged(boolean connected, String message);
        void onStatus(JSONObject status);
    }

    private final Handler mainHandler = new Handler(Looper.getMainLooper());
    private final ExecutorService cmdExecutor = Executors.newSingleThreadExecutor();
    private final Listener listener;

    private volatile Mode mode = Mode.BLUETOOTH;
    private volatile String btMac = "";
    private volatile String btName = "";
    private volatile String host = "127.0.0.1";
    private volatile int port = 26263;
    private volatile String authToken = "";

    private volatile boolean running = false;
    private volatile boolean connected = false;
    private volatile int generation = 0;

    // TCP state
    private volatile Socket subSocket;
    private Socket cmdSocket;
    private BufferedReader cmdReader;
    private BufferedWriter cmdWriter;
    private final Object cmdLock = new Object();

    // Bluetooth RFCOMM single-socket multiplexed state
    private volatile BluetoothSocket btSocket;
    private BufferedWriter btWriter;
    private final Object btWriteLock = new Object();
    private final ConcurrentLinkedQueue<Consumer<JSONObject>> btPendingCallbacks = new ConcurrentLinkedQueue<>();

    private Thread workerThread;

    public TntronRemoteClient(Listener listener) {
        this.listener = listener;
    }

    public boolean isConnected() {
        return connected;
    }

    public Mode getMode() {
        return mode;
    }

    public void connectBluetooth(String mac, String name) {
        this.mode = Mode.BLUETOOTH;
        this.btMac = mac == null ? "" : mac.trim();
        this.btName = (name == null || name.trim().isEmpty()) ? this.btMac : name.trim();
        restartLoop();
    }

    public void configureAndConnect(String host, int port, String authToken) {
        this.mode = Mode.TCP;
        this.host = (host == null || host.trim().isEmpty()) ? "127.0.0.1" : host.trim();
        this.port = (port < 1024 || port > 65535) ? 26263 : port;
        this.authToken = authToken == null ? "" : authToken.trim();
        restartLoop();
    }

    private synchronized void restartLoop() {
        generation++;
        stopSocketsOnly();
        if (!running) {
            running = true;
            workerThread = new Thread(this::connectionLoop, "tntron-android-link");
            workerThread.setDaemon(true);
            workerThread.start();
        }
    }

    public synchronized void shutdown() {
        running = false;
        generation++;
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
        synchronized (btWriteLock) {
            closeQuietBt(btSocket);
            btSocket = null;
            btWriter = null;
            btPendingCallbacks.clear();
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

    private static void closeQuietBt(BluetoothSocket s) {
        if (s != null) {
            try { s.close(); } catch (IOException ignored) {}
        }
    }

    private void connectionLoop() {
        while (running) {
            int myGen = generation;
            if (mode == Mode.BLUETOOTH) {
                runBluetoothSession(myGen);
            } else {
                runTcpSession(myGen);
            }
            if (running && myGen == generation) {
                try {
                    Thread.sleep(2200);
                } catch (InterruptedException ignored) {
                    break;
                }
            }
        }
    }

    @SuppressLint("MissingPermission")
    private BluetoothSocket openRfcommSocket(BluetoothDevice dev) throws IOException {
        // Prefer direct RFCOMM channel 22 (matches tntron-core bind_rfcomm_listener(22))
        try {
            Method m = dev.getClass().getMethod("createRfcommSocket", int.class);
            BluetoothSocket s = (BluetoothSocket) m.invoke(dev, BT_RFCOMM_CHANNEL);
            if (s != null) {
                s.connect();
                return s;
            }
        } catch (Throwable ignored) {}

        try {
            Method m = dev.getClass().getMethod("createInsecureRfcommSocket", int.class);
            BluetoothSocket s = (BluetoothSocket) m.invoke(dev, BT_RFCOMM_CHANNEL);
            if (s != null) {
                s.connect();
                return s;
            }
        } catch (Throwable ignored) {}

        BluetoothSocket spp = dev.createRfcommSocketToServiceRecord(SPP_UUID);
        spp.connect();
        return spp;
    }

    @SuppressLint("MissingPermission")
    private void runBluetoothSession(int myGen) {
        if (btMac == null || btMac.isEmpty()) {
            notifyConnection(false, "Select a paired PC in Connect tab");
            return;
        }
        BluetoothAdapter adapter = BluetoothAdapter.getDefaultAdapter();
        if (adapter == null || !adapter.isEnabled()) {
            notifyConnection(false, "Bluetooth is OFF on this device");
            return;
        }

        try {
            notifyConnection(false, "Bluetooth connecting to " + btName + "...");
            BluetoothDevice dev = adapter.getRemoteDevice(btMac);
            BluetoothSocket sock = openRfcommSocket(dev);
            if (myGen != generation || !running) {
                closeQuietBt(sock);
                return;
            }
            BufferedReader reader = new BufferedReader(new InputStreamReader(sock.getInputStream(), StandardCharsets.UTF_8));
            BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(sock.getOutputStream(), StandardCharsets.UTF_8));
            synchronized (btWriteLock) {
                btSocket = sock;
                btWriter = writer;
                btPendingCallbacks.clear();
            }

            connected = true;
            notifyConnection(true, "Bluetooth · " + btName);

            String line;
            while (running && myGen == generation && (line = reader.readLine()) != null) {
                if (line.trim().isEmpty()) continue;
                try {
                    JSONObject obj = new JSONObject(line);
                    if (obj.has("ok")) {
                        Consumer<JSONObject> cb = btPendingCallbacks.poll();
                        if (cb != null) {
                            mainHandler.post(() -> cb.accept(obj));
                        }
                        if (obj.has("status") && !obj.isNull("status")) {
                            JSONObject st = obj.optJSONObject("status");
                            if (st != null) mainHandler.post(() -> listener.onStatus(st));
                        }
                    } else if (obj.has("state")) {
                        mainHandler.post(() -> listener.onStatus(obj));
                    }
                } catch (Exception ignored) {}
            }
        } catch (Exception e) {
            if (myGen == generation) {
                String msg = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
                notifyConnection(false, "BT Offline: " + msg);
            }
        } finally {
            connected = false;
            synchronized (btWriteLock) {
                closeQuietBt(btSocket);
                btSocket = null;
                btWriter = null;
                btPendingCallbacks.clear();
            }
        }
    }

    private Socket openTcpSocket() throws IOException {
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

    private void runTcpSession(int myGen) {
        try {
            notifyConnection(false, "TCP connecting to " + host + ":" + port + "...");
            Socket s = openTcpSocket();
            if (myGen != generation || !running) {
                closeQuiet(s);
                return;
            }
            subSocket = s;
            BufferedReader reader = new BufferedReader(new InputStreamReader(s.getInputStream(), StandardCharsets.UTF_8));
            BufferedWriter writer = new BufferedWriter(new OutputStreamWriter(s.getOutputStream(), StandardCharsets.UTF_8));

            authIfNeeded(writer, reader);
            writer.write("{\"cmd\":\"subscribe\"}\n");
            writer.flush();

            connected = true;
            notifyConnection(true, "TCP · " + host + ":" + port);

            String line;
            while (running && myGen == generation && (line = reader.readLine()) != null) {
                if (line.trim().isEmpty()) continue;
                try {
                    JSONObject st = new JSONObject(line);
                    mainHandler.post(() -> listener.onStatus(st));
                } catch (Exception ignored) {}
            }
        } catch (Exception e) {
            if (myGen == generation) {
                String msg = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
                notifyConnection(false, "TCP Offline: " + msg);
            }
        } finally {
            connected = false;
            closeQuiet(subSocket);
            subSocket = null;
        }
    }

    private void notifyConnection(boolean isConn, String msg) {
        mainHandler.post(() -> listener.onConnectionChanged(isConn, msg));
    }

    private JSONObject sendTcpSync(JSONObject payload) {
        synchronized (cmdLock) {
            for (int attempt = 0; attempt < 2; attempt++) {
                try {
                    if (cmdSocket == null || cmdSocket.isClosed() || !cmdSocket.isConnected()) {
                        cmdSocket = openTcpSocket();
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
        if (mode == Mode.BLUETOOTH) {
            cmdExecutor.submit(() -> {
                synchronized (btWriteLock) {
                    if (btWriter == null) {
                        if (onDone != null) mainHandler.post(() -> onDone.accept(null));
                        return;
                    }
                    try {
                        btPendingCallbacks.add(onDone != null ? onDone : r -> {});
                        btWriter.write(payload.toString());
                        btWriter.write('\n');
                        btWriter.flush();
                    } catch (IOException e) {
                        btPendingCallbacks.clear();
                        closeQuietBt(btSocket);
                        btSocket = null;
                        btWriter = null;
                        if (onDone != null) mainHandler.post(() -> onDone.accept(null));
                    }
                }
            });
        } else {
            cmdExecutor.submit(() -> {
                JSONObject resp = sendTcpSync(payload);
                if (onDone != null) {
                    mainHandler.post(() -> onDone.accept(resp));
                }
            });
        }
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
