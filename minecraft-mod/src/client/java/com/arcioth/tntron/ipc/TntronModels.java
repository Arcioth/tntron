package com.arcioth.tntron.ipc;

import java.util.Collections;
import java.util.List;

public final class TntronModels {
    private TntronModels() {}

    public static class PopupTrackItem {
        public String title = "";
        public String artist = "";
        public double duration = 0.0;
        public Integer playlist_index;
        public boolean is_current = false;
        public boolean is_queued = false;
    }

    public static class McNotice {
        public long seq = 0;
        public String kind = "toast";
        public String title = "";
        public String body = "";
    }

    public static class TntronStatus {
        public String state = "stopped";
        public String title = "No Track";
        public String artist = "";
        public String album = "";
        public String track_path = "";
        public String cover_path;
        public double position = 0.0;
        public double duration = 0.0;
        public double volume = 100.0;
        public double speed = 1.0;
        public Double bpm;
        public double energy = 0.0;
        public float[] bands = new float[16];
        public float beat = 0.0f;
        public String lyric = "";
        public List<String> lyrics_window = Collections.emptyList();
        public List<PopupTrackItem> playlist_items = Collections.emptyList();
        public List<PopupTrackItem> queue_items = Collections.emptyList();
        public String loop_mode = "All";
        public boolean shuffle = false;
        public String dsp_preset = "Flat";
        public boolean spatial_on = false;
        public Long sleep_remaining_sec;
        public String text = "";
        public String alt = "";
        public String tooltip = "";
        public String class_name = "";
        public int percentage = 0;

        public boolean muted = false;
        public double pitch_st = 0.0;
        public boolean link_pitch = false;
        public boolean preserve_formant = true;
        public float spatial_hz = 0.08f;
        public int bass_db = 0;
        public int mid_db = 0;
        public int treble_db = 0;
        public Double ab_loop_a;
        public Double ab_loop_b;
        public boolean is_favorite = false;
        public String codec;
        public Integer bitrate_kbps;
        public Integer sample_rate_hz;
        public Integer channels;
        public double peak_db = -60.0;
        public String active_theme = "openify";
        public boolean hide_ui = false;
        public String current_dir = "";
        public boolean mc_link_enabled = false;
        public McNotice mc_notice;

        public boolean isPlaying() {
            return "playing".equalsIgnoreCase(state);
        }

        public boolean isPaused() {
            return "paused".equalsIgnoreCase(state);
        }

        public boolean hasTrack() {
            return title != null && !title.isEmpty() && !"No Track".equals(title) && !"stopped".equalsIgnoreCase(state);
        }
    }

    public static class LibraryFolder {
        public String name = "";
        public String path = "";
        public int track_count = 0;
    }

    public static class LibraryTrack {
        public String title = "";
        public String artist = "";
        public String album = "";
        public String path = "";
        public double duration = 0.0;
        public Double bpm;
        public boolean is_favorite = false;
    }

    public static class LibraryData {
        public String current_dir = "";
        public List<LibraryFolder> folders = Collections.emptyList();
        public List<LibraryTrack> tracks = Collections.emptyList();
        public int total_tracks = 0;
    }

    public static class ThemeItem {
        public String slug = "";
        public String label = "";
        public String accent = "#1db954";
        public boolean is_extension = false;
    }

    public static class ExtensionItem {
        public String id = "";
        public String name = "";
        public String version = "";
        public String description = "";
        public boolean enabled = false;
        public String error;
    }

    public static class IpcResponse {
        public boolean ok = false;
        public String error;
        public TntronStatus status;
        public LibraryData library;
        public List<ThemeItem> themes;
        public List<ExtensionItem> extensions;
        public String cover_base64;
        public String message;
    }

    public static class McTelemetry {
        public String cmd = "mc_telemetry";
        public String client_id = "fabric-26.2";
        public String player_name = "";
        public String dimension = "";
        public String biome = "";
        public float health = 20.0f;
        public float max_health = 20.0f;
        public boolean in_combat = false;
        public boolean boss_active = false;
        public String boss_name;
        public boolean paused = false;
        public boolean in_world = false;
        public String weather = "clear";
        public boolean underwater = false;
        public float speed_bps = 0.0f;
        public boolean tnt_nearby = false;
        public long game_time = 0;
    }
}
