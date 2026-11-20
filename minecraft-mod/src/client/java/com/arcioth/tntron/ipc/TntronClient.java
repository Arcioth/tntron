package com.arcioth.tntron.ipc;

import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronModels.*;
import com.google.gson.Gson;
import com.google.gson.JsonObject;
import net.minecraft.client.Minecraft;

import java.io.*;
import java.net.InetSocketAddress;
import java.net.StandardProtocolFamily;
import java.net.UnixDomainSocketAddress;
import java.nio.channels.Channels;
import java.nio.channels.SocketChannel;
import java.nio.charset.StandardCharsets;
import java.nio.file.Files;
import java.nio.file.Path;
import java.util.Collections;
import java.util.List;
import java.util.concurrent.*;
import java.util.function.Consumer;

public final class TntronClient {
    private static final Gson GSON = new Gson();

    private final TntronModConfig config;
    private final ExecutorService cmdExecutor = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "tntron-ipc-cmd");
        t.setDaemon(true);
        return t;
    });

    private volatile boolean running = true;
    private volatile boolean connected = false;
    private volatile String transportLabel = "Offline";
    private volatile String lastError = "";
    private volatile TntronStatus latestStatus = new TntronStatus();
    private volatile LibraryData cachedLibrary = new LibraryData();
    private volatile List<ThemeItem> cachedThemes = Collections.emptyList();
    private volatile List<ExtensionItem> cachedExtensions = Collections.emptyList();

    private volatile SocketChannel subChannel;
    private volatile SocketChannel cmdChannel;
    private BufferedReader cmdReader;
    private BufferedWriter cmdWriter;
    private final Object cmdLock = new Object();

    private final Consumer<TntronStatus> onStatusCallback;
    private Thread subThread;

    public TntronClient(TntronModConfig config, Consumer<TntronStatus> onStatusCallback) {
        this.config = config;
        this.onStatusCallback = onStatusCallback;
    }

    public void start() {
        running = true;
        subThread = new Thread(this::subscriberLoop, "tntron-ipc-sub");
        subThread.setDaemon(true);
        subThread.start();
    }

    public void stop() {
        running = false;
        closeChannelQuiet(subChannel);
        synchronized (cmdLock) {
            closeCmdChannel();
        }
        cmdExecutor.shutdownNow();
    }

    public void reconnect() {
        connected = false;
        closeChannelQuiet(subChannel);
        synchronized (cmdLock) {
            closeCmdChannel();
        }
    }

    public boolean isConnected() {
        return connected;
    }

    public String getTransportLabel() {
        return transportLabel;
    }

    public String getLastError() {
        return lastError;
    }

    public TntronStatus getStatus() {
        return latestStatus;
    }

    public LibraryData getCachedLibrary() {
        return cachedLibrary;
    }

    public List<ThemeItem> getCachedThemes() {
        return cachedThemes;
    }

    public List<ExtensionItem> getCachedExtensions() {
        return cachedExtensions;
    }

    private static Path resolveUnixSocketPath() {
        String xdg = System.getenv("XDG_RUNTIME_DIR");
        if (xdg != null && !xdg.isBlank()) {
            return Path.of(xdg, "tntron.sock");
        }
        String user = System.getProperty("user.name", "1000");
        Path byUid = Path.of("/tmp/tntron-" + user + ".sock");
        if (Files.exists(byUid)) return byUid;
        try {
            for (Path p : Files.newDirectoryStream(Path.of("/tmp"), "tntron-*.sock")) {
                return p;
            }
        } catch (IOException ignored) {
        }
        return Path.of("/tmp/tntron-1000.sock");
    }

    private record ChannelPair(SocketChannel channel, String label) {}

    private ChannelPair openBestChannel() throws IOException {
        String mode = config.connectionMode == null ? "auto" : config.connectionMode.toLowerCase();
        IOException lastEx = null;

        if ("auto".equals(mode) || "tcp".equals(mode)) {
            try {
                SocketChannel sc = SocketChannel.open(StandardProtocolFamily.INET);
                sc.socket().connect(new InetSocketAddress(config.tcpHost, config.tcpPort), 1200);
                sc.socket().setTcpNoDelay(true);
                return new ChannelPair(sc, "TCP " + config.tcpHost + ":" + config.tcpPort);
            } catch (IOException e) {
                lastEx = e;
                if ("tcp".equals(mode)) throw e;
            }
        }

        if ("auto".equals(mode) || "unix".equals(mode)) {
            Path sock = resolveUnixSocketPath();
            if (!Files.exists(sock)) {
                throw new IOException(lastEx != null
                        ? ("TCP refused & Unix socket missing (" + sock + ")")
                        : ("Unix socket not found: " + sock));
            }
            SocketChannel sc = SocketChannel.open(StandardProtocolFamily.UNIX);
            sc.connect(UnixDomainSocketAddress.of(sock));
            return new ChannelPair(sc, "Unix IPC");
        }

        throw lastEx != null ? lastEx : new IOException("No transport available");
    }

    private void authenticateIfNeeded(BufferedWriter writer, BufferedReader reader) throws IOException {
        if (config.authToken != null && !config.authToken.isBlank()) {
            JsonObject auth = new JsonObject();
            auth.addProperty("cmd", "auth");
            auth.addProperty("token", config.authToken.trim());
            writer.write(auth.toString());
            writer.write('\n');
            writer.flush();
            String respLine = reader.readLine();
            if (respLine == null) throw new EOFException("Closed during auth");
            IpcResponse resp = GSON.fromJson(respLine, IpcResponse.class);
            if (resp == null || !resp.ok) {
                throw new IOException(resp != null && resp.error != null ? resp.error : "Auth failed");
            }
        }
    }

    private void subscriberLoop() {
        while (running) {
            try {
                ChannelPair pair = openBestChannel();
                subChannel = pair.channel();
                BufferedReader reader = new BufferedReader(
                        new InputStreamReader(Channels.newInputStream(subChannel), StandardCharsets.UTF_8));
                BufferedWriter writer = new BufferedWriter(
                        new OutputStreamWriter(Channels.newOutputStream(subChannel), StandardCharsets.UTF_8));

                authenticateIfNeeded(writer, reader);

                writer.write("{\"cmd\":\"subscribe\"}\n");
                writer.flush();

                connected = true;
                transportLabel = pair.label();
                lastError = "";

                // Trigger initial metadata refreshes
                refreshThemes(null);
                refreshExtensions(null);

                String line;
                while (running && (line = reader.readLine()) != null) {
                    if (line.isBlank()) continue;
                    try {
                        TntronStatus st = GSON.fromJson(line, TntronStatus.class);
                        if (st != null) {
                            latestStatus = st;
                            if (onStatusCallback != null) {
                                onStatusCallback.accept(st);
                            }
                        }
                    } catch (Exception ignored) {
                    }
                }
            } catch (Exception e) {
                lastError = e.getMessage() != null ? e.getMessage() : e.getClass().getSimpleName();
            } finally {
                connected = false;
                transportLabel = "Offline";
                closeChannelQuiet(subChannel);
                subChannel = null;
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

    private void ensureCmdChannel() throws IOException {
        if (cmdChannel != null && cmdChannel.isOpen()) return;
        ChannelPair pair = openBestChannel();
        cmdChannel = pair.channel();
        cmdReader = new BufferedReader(
                new InputStreamReader(Channels.newInputStream(cmdChannel), StandardCharsets.UTF_8));
        cmdWriter = new BufferedWriter(
                new OutputStreamWriter(Channels.newOutputStream(cmdChannel), StandardCharsets.UTF_8));
        authenticateIfNeeded(cmdWriter, cmdReader);
    }

    private void closeCmdChannel() {
        closeChannelQuiet(cmdChannel);
        cmdChannel = null;
        cmdReader = null;
        cmdWriter = null;
    }

    private static void closeChannelQuiet(SocketChannel ch) {
        if (ch != null) {
            try {
                ch.close();
            } catch (IOException ignored) {
            }
        }
    }

    private IpcResponse sendSyncJson(String jsonLine) {
        synchronized (cmdLock) {
            for (int attempt = 0; attempt < 2; attempt++) {
                try {
                    ensureCmdChannel();
                    cmdWriter.write(jsonLine);
                    cmdWriter.write('\n');
                    cmdWriter.flush();
                    String respLine = cmdReader.readLine();
                    if (respLine == null) {
                        throw new EOFException("EOF on IPC command socket");
                    }
                    IpcResponse resp = GSON.fromJson(respLine, IpcResponse.class);
                    if (resp != null && resp.status != null) {
                        latestStatus = resp.status;
                    }
                    return resp;
                } catch (Exception e) {
                    lastError = e.getMessage() != null ? e.getMessage() : "IPC error";
                    closeCmdChannel();
                }
            }
            return null;
        }
    }

    public void sendSimple(String cmd) {
        JsonObject obj = new JsonObject();
        obj.addProperty("cmd", cmd);
        sendJsonAsync(obj, null);
    }

    public void sendJsonAsync(JsonObject payload, Consumer<IpcResponse> onDone) {
        if (cmdExecutor.isShutdown()) return;
        cmdExecutor.submit(() -> {
            IpcResponse resp = sendSyncJson(payload.toString());
            if (onDone != null) {
                Minecraft mc = Minecraft.getInstance();
                mc.execute(() -> onDone.accept(resp));
            }
        });
    }

    // --- Transport & Audio Controls ---
    public void play() { sendSimple("play"); }
    public void pause() { sendSimple("pause"); }
    public void toggle() { sendSimple("toggle"); }
    public void stopPlayback() { sendSimple("stop"); }
    public void next() { sendSimple("next"); }
    public void prev() { sendSimple("prev"); }
    public void cycleLoop() { sendSimple("cycle_loop"); }
    public void toggleShuffle() { sendSimple("shuffle"); }
    public void toggleSpatial() { sendSimple("toggle_spatial"); }
    public void toggleHideUi() { sendSimple("toggle_hide_ui"); }
    public void rescan() { sendSimple("rescan"); }
    public void clearQueue() { sendSimple("clear_queue"); }

    public void setVolume(double value) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "volume");
        o.addProperty("value", Math.max(0.0, Math.min(100.0, value)));
        sendJsonAsync(o, null);
    }

    public void volumeDelta(int delta) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "volume");
        o.addProperty("delta", delta);
        sendJsonAsync(o, null);
    }

    public void setMute(Boolean muted) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "mute");
        if (muted != null) o.addProperty("value", muted);
        sendJsonAsync(o, null);
    }

    public void seekAbsolute(double sec) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "seek");
        o.addProperty("value", Math.max(0.0, sec));
        sendJsonAsync(o, null);
    }

    public void seekDelta(long deltaSec) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "seek");
        o.addProperty("delta", deltaSec);
        sendJsonAsync(o, null);
    }

    public void setSpeed(double speed) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "speed");
        o.addProperty("value", Math.max(0.25, Math.min(3.0, speed)));
        sendJsonAsync(o, null);
    }

    public void setPitch(Double semitones, Double delta, Boolean linkPitch, Boolean preserveFormant) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "pitch");
        if (semitones != null) o.addProperty("value", semitones);
        if (delta != null) o.addProperty("delta", delta);
        if (linkPitch != null) o.addProperty("link_pitch", linkPitch);
        if (preserveFormant != null) o.addProperty("preserve_formant", preserveFormant);
        sendJsonAsync(o, null);
    }

    public void setDspPreset(String preset) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "dsp_preset");
        o.addProperty("preset", preset);
        sendJsonAsync(o, null);
    }

    public void stepEq(String band, int delta) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", switch (band) {
            case "bass" -> "step_bass";
            case "mid" -> "step_mid";
            default -> "step_treble";
        });
        o.addProperty("delta", delta);
        sendJsonAsync(o, null);
    }

    public void stepSpatialHz(float deltaHz) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "set_spatial");
        o.addProperty("delta_hz", deltaHz);
        sendJsonAsync(o, null);
    }

    public void abLoop(String action) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "ab_loop");
        o.addProperty("action", action);
        sendJsonAsync(o, null);
    }

    public void toggleFavorite(String pathOrNull) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "toggle_favorite");
        if (pathOrNull != null && !pathOrNull.isEmpty()) o.addProperty("path", pathOrNull);
        sendJsonAsync(o, null);
    }

    public void setSleepMinutes(Integer minutes) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "sleep");
        if (minutes != null) o.addProperty("minutes", minutes);
        sendJsonAsync(o, null);
    }

    public void stepSleepMinutes(long delta) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "sleep_step");
        o.addProperty("delta_minutes", delta);
        sendJsonAsync(o, null);
    }

    public void setTheme(String slug) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "set_theme");
        o.addProperty("slug", slug);
        sendJsonAsync(o, null);
    }

    public void playPath(String path) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_path");
        o.addProperty("path", path);
        sendJsonAsync(o, null);
    }

    public void playFolder(String path, boolean shuffle) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_folder");
        o.addProperty("path", path);
        o.addProperty("shuffle", shuffle);
        sendJsonAsync(o, null);
    }

    public void playFavorites(boolean shuffle) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_favorites");
        o.addProperty("shuffle", shuffle);
        sendJsonAsync(o, null);
    }

    public void playAllLibrary(boolean shuffle) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_all_library");
        o.addProperty("shuffle", shuffle);
        sendJsonAsync(o, null);
    }

    public void playUpcoming(int index) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_upcoming");
        o.addProperty("index", index);
        sendJsonAsync(o, null);
    }

    public void playQueueItem(int index) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_queue_item");
        o.addProperty("index", index);
        sendJsonAsync(o, null);
    }

    public void addToQueue(String path) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "add_to_queue");
        o.addProperty("path", path);
        sendJsonAsync(o, null);
    }

    public void playNextInQueue(String path) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "play_next_in_queue");
        o.addProperty("path", path);
        sendJsonAsync(o, null);
    }

    public void addFolderToQueue(String path) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "add_folder_to_queue");
        o.addProperty("path", path);
        sendJsonAsync(o, null);
    }

    public void removeFromQueue(int index) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "remove_from_queue");
        o.addProperty("index", index);
        sendJsonAsync(o, null);
    }

    public void toggleExtension(String id) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "toggle_extension");
        o.addProperty("id", id);
        sendJsonAsync(o, r -> refreshExtensions(null));
    }

    public void sendTelemetry(McTelemetry telemetry) {
        if (!connected || cmdExecutor.isShutdown()) return;
        cmdExecutor.submit(() -> sendSyncJson(GSON.toJson(telemetry)));
    }

    public void sendMcEvent(String event, String detail) {
        if (!connected) return;
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "mc_event");
        o.addProperty("event", event);
        if (detail != null) o.addProperty("detail", detail);
        sendJsonAsync(o, null);
    }

    public void translateLyrics(String lang) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "translate_lyrics");
        o.addProperty("lang", (lang == null || lang.isBlank()) ? "en" : lang.trim().toLowerCase());
        sendJsonAsync(o, null);
    }

    public void queryLibrary(String query, String folder, int limit, Consumer<LibraryData> callback) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "get_library");
        if (query != null && !query.isBlank()) o.addProperty("query", query.trim());
        if (folder != null && !folder.isBlank()) o.addProperty("folder", folder.trim());
        o.addProperty("limit", limit);
        sendJsonAsync(o, resp -> {
            if (resp != null && resp.ok && resp.library != null) {
                cachedLibrary = resp.library;
                if (callback != null) callback.accept(resp.library);
            } else if (callback != null) {
                callback.accept(cachedLibrary);
            }
        });
    }

    public void refreshThemes(Consumer<List<ThemeItem>> callback) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "get_themes");
        sendJsonAsync(o, resp -> {
            if (resp != null && resp.ok && resp.themes != null) {
                cachedThemes = resp.themes;
            }
            if (callback != null) callback.accept(cachedThemes);
        });
    }

    public void refreshExtensions(Consumer<List<ExtensionItem>> callback) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "get_extensions");
        sendJsonAsync(o, resp -> {
            if (resp != null && resp.ok && resp.extensions != null) {
                cachedExtensions = resp.extensions;
            }
            if (callback != null) callback.accept(cachedExtensions);
        });
    }

    public void fetchCoverBase64(Consumer<String> callback) {
        JsonObject o = new JsonObject();
        o.addProperty("cmd", "get_cover");
        sendJsonAsync(o, resp -> {
            if (callback != null) {
                callback.accept(resp != null ? resp.cover_base64 : null);
            }
        });
    }
}
