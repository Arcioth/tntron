package com.arcioth.tntron.config;

import com.google.gson.Gson;
import com.google.gson.GsonBuilder;
import net.fabricmc.loader.api.FabricLoader;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;

public final class TntronModConfig {
    private static final Gson GSON = new GsonBuilder().setPrettyPrinting().create();
    private static final Path CONFIG_PATH = FabricLoader.getInstance().getConfigDir().resolve("tntron-mc.json");

    // Connection
    public String connectionMode = "auto"; // "auto" | "tcp" | "unix"
    public String tcpHost = "127.0.0.1";
    public int tcpPort = 26262;
    public String authToken = "";

    // HUD
    public boolean hudEnabled = true;
    public String hudPosition = "top_left"; // "top_left" | "top_right" | "bottom_left" | "bottom_right"
    public float hudScale = 1.0f;
    public float hudOpacity = 0.80f;
    public boolean showCover = true;
    public boolean showSpectrum = true;
    public boolean showProgress = true;
    public boolean showMetaBadges = true;
    public boolean lyricsOverlayEnabled = true;
    public String lyricsDisplayMode = "both"; // "original" | "translated" | "both"
    public String lyricsTranslateLang = "en";
    public int lyricsYOffset = 62;

    // Reactive Visual & World Effects
    public boolean reactiveEnabled = true;
    public boolean beatVignette = true;
    public boolean beatFovPulse = false;
    public boolean beatParticles = true;
    public float beatIntensity = 0.65f;

    // Game Audio & Notifications
    public boolean autoMuteMcMusic = true;
    public double savedMcMusicVolume = -1.0;
    public String notifyMode = "toast"; // "toast" | "actionbar" | "chat" | "off"
    public boolean sendTelemetry = true;

    // Theme
    public boolean syncAccentWithTheme = true;
    public String customAccentHex = "#1db954";

    public static TntronModConfig load() {
        if (Files.isRegularFile(CONFIG_PATH)) {
            try {
                String json = Files.readString(CONFIG_PATH);
                TntronModConfig loaded = GSON.fromJson(json, TntronModConfig.class);
                if (loaded != null) {
                    loaded.clamp();
                    return loaded;
                }
            } catch (Exception ignored) {
            }
        }
        TntronModConfig def = new TntronModConfig();
        def.save();
        return def;
    }

    public void clamp() {
        if (tcpHost == null || tcpHost.isBlank()) tcpHost = "127.0.0.1";
        if (tcpPort < 1024 || tcpPort > 65535) tcpPort = 26262;
        if (authToken == null) authToken = "";
        if (connectionMode == null || connectionMode.isBlank()) connectionMode = "auto";
        if (hudPosition == null || hudPosition.isBlank()) hudPosition = "top_left";
        if (notifyMode == null || notifyMode.isBlank()) notifyMode = "toast";
        if (lyricsDisplayMode == null) lyricsDisplayMode = "both";
        if (!"original".equals(lyricsDisplayMode)
                && !"translated".equals(lyricsDisplayMode)
                && !"both".equals(lyricsDisplayMode)) {
            lyricsDisplayMode = "both";
        }
        if (lyricsTranslateLang == null || lyricsTranslateLang.isBlank()) lyricsTranslateLang = "en";
        if (customAccentHex == null || customAccentHex.isBlank()) customAccentHex = "#1db954";
        hudScale = Math.max(0.6f, Math.min(1.6f, hudScale));
        hudOpacity = Math.max(0.25f, Math.min(1.0f, hudOpacity));
        beatIntensity = Math.max(0.0f, Math.min(1.5f, beatIntensity));
        lyricsYOffset = Math.max(36, Math.min(160, lyricsYOffset));
    }

    public String lyricsDisplayLabel() {
        return switch (lyricsDisplayMode) {
            case "original" -> "Original";
            case "translated" -> "Translated";
            default -> "Both";
        };
    }

    public synchronized void save() {
        clamp();
        try {
            Files.createDirectories(CONFIG_PATH.getParent());
            Files.writeString(CONFIG_PATH, GSON.toJson(this));
        } catch (IOException ignored) {
        }
    }
}
