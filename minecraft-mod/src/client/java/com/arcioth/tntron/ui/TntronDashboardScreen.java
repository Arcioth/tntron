package com.arcioth.tntron.ui;

import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronClient;
import com.arcioth.tntron.ipc.TntronModels.*;
import com.arcioth.tntron.render.CoverArtManager;
import com.arcioth.tntron.render.TntronThemeColors;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.gui.components.AbstractSliderButton;
import net.minecraft.client.gui.components.Button;
import net.minecraft.client.gui.components.EditBox;
import net.minecraft.client.gui.screens.Screen;
import net.minecraft.client.input.KeyEvent;
import net.minecraft.client.input.MouseButtonEvent;
import net.minecraft.client.renderer.RenderPipelines;
import net.minecraft.network.chat.Component;
import org.lwjgl.glfw.GLFW;

import java.util.ArrayList;
import java.util.List;

public final class TntronDashboardScreen extends Screen {
    private enum Tab {
        NOW_PLAYING,
        DSP_AUDIO,
        LIBRARY_QUEUE,
        THEMES_CONFIG
    }

    private enum LibrarySubTab {
        TRACKS,
        FOLDERS,
        QUEUE
    }

    private enum ConfigSubTab {
        THEMES,
        EXTENSIONS,
        MOD_SETTINGS
    }

    private final TntronClient client;
    private final TntronModConfig config;
    private final CoverArtManager coverArt;

    private Tab activeTab = Tab.NOW_PLAYING;
    private LibrarySubTab libSubTab = LibrarySubTab.TRACKS;
    private ConfigSubTab cfgSubTab = ConfigSubTab.THEMES;

    private int panelX, panelY, panelW, panelH;
    private int seekBarX, seekBarY, seekBarW, seekBarH;

    private String searchQuery = "";
    private String selectedFolder = "";
    private int scrollOffset = 0;

    private EditBox searchBox;
    private EditBox portBox;
    private final List<Button> rowActionButtons = new ArrayList<>();
    private int lastStatusHash = 0;

    public TntronDashboardScreen(TntronClient client, TntronModConfig config, CoverArtManager coverArt) {
        super(Component.literal("tntron Control Center"));
        this.client = client;
        this.config = config;
        this.coverArt = coverArt;
    }

    @Override
    public boolean isPauseScreen() {
        return false;
    }

    @Override
    protected void init() {
        super.init();
        panelW = Math.min(540, this.width - 16);
        panelH = Math.min(340, this.height - 16);
        panelX = (this.width - panelW) / 2;
        panelY = (this.height - panelH) / 2;

        rebuildWidgets();

        if (activeTab == Tab.LIBRARY_QUEUE && client.getCachedLibrary().tracks.isEmpty()) {
            client.queryLibrary(searchQuery, selectedFolder, 120, lib -> {
                if (activeTab == Tab.LIBRARY_QUEUE) rebuildWidgets();
            });
        } else if (activeTab == Tab.THEMES_CONFIG) {
            client.refreshThemes(t -> {
                if (activeTab == Tab.THEMES_CONFIG && cfgSubTab == ConfigSubTab.THEMES) rebuildWidgets();
            });
            client.refreshExtensions(e -> {
                if (activeTab == Tab.THEMES_CONFIG && cfgSubTab == ConfigSubTab.EXTENSIONS) rebuildWidgets();
            });
        }
    }

    @Override
    protected void rebuildWidgets() {
        clearWidgets();
        rowActionButtons.clear();
        searchBox = null;
        portBox = null;

        // Top Header controls
        int headerBtnY = panelY + 8;
        addRenderableWidget(Button.builder(Component.literal("Reconnect"), b -> client.reconnect())
                .bounds(panelX + panelW - 92, headerBtnY, 64, 18).build());
        addRenderableWidget(Button.builder(Component.literal("✕"), b -> onClose())
                .bounds(panelX + panelW - 24, headerBtnY, 18, 18).build());

        // Navigation Tabs
        int tabY = panelY + 32;
        int tabW = (panelW - 24 - 12) / 4;
        String[] tabLabels = {"Now Playing", "DSP & Audio", "Library & Queue", "Themes & Config"};
        Tab[] tabs = Tab.values();
        for (int i = 0; i < tabs.length; i++) {
            final Tab t = tabs[i];
            String lbl = (t == activeTab ? "▸ " : "") + tabLabels[i];
            addRenderableWidget(Button.builder(Component.literal(lbl), b -> {
                activeTab = t;
                scrollOffset = 0;
                if (t == Tab.LIBRARY_QUEUE) {
                    client.queryLibrary(searchQuery, selectedFolder, 120, lib -> rebuildWidgets());
                } else if (t == Tab.THEMES_CONFIG) {
                    client.refreshThemes(th -> rebuildWidgets());
                    client.refreshExtensions(ex -> rebuildWidgets());
                }
                rebuildWidgets();
            }).bounds(panelX + 12 + i * (tabW + 4), tabY, tabW, 20).build());
        }

        switch (activeTab) {
            case NOW_PLAYING -> buildNowPlayingWidgets();
            case DSP_AUDIO -> buildDspWidgets();
            case LIBRARY_QUEUE -> buildLibraryWidgets();
            case THEMES_CONFIG -> buildThemesConfigWidgets();
        }
    }

    private void buildNowPlayingWidgets() {
        TntronStatus st = client.getStatus();
        int bodyX = panelX + 12;
        int transportY = panelY + 156;

        // Primary Transport Row
        int bw = 66;
        int gap = 4;
        addRenderableWidget(Button.builder(Component.literal("⏮ Prev"), b -> client.prev())
                .bounds(bodyX, transportY, bw, 20).build());
        addRenderableWidget(Button.builder(Component.literal(st.isPlaying() ? "⏸ Pause" : "▶ Play"), b -> client.toggle())
                .bounds(bodyX + (bw + gap), transportY, bw + 10, 20).build());
        addRenderableWidget(Button.builder(Component.literal("■ Stop"), b -> client.stopPlayback())
                .bounds(bodyX + (bw + gap) * 2 + 10, transportY, bw - 8, 20).build());
        addRenderableWidget(Button.builder(Component.literal("⏭ Next"), b -> client.next())
                .bounds(bodyX + (bw + gap) * 3 + 2, transportY, bw, 20).build());
        addRenderableWidget(Button.builder(Component.literal("🔀 " + (st.shuffle ? "ON" : "OFF")), b -> client.toggleShuffle())
                .bounds(bodyX + (bw + gap) * 4 + 2, transportY, 64, 20).build());
        addRenderableWidget(Button.builder(Component.literal("🔁 " + st.loop_mode), b -> client.cycleLoop())
                .bounds(bodyX + (bw + gap) * 5 + 2, transportY, 66, 20).build());
        addRenderableWidget(Button.builder(Component.literal(st.is_favorite ? "♥ Fav" : "♡ Fav"), b -> client.toggleFavorite(null))
                .bounds(bodyX + (bw + gap) * 6 + 4, transportY, 54, 20).build());

        // Quick Seek & Mute Row
        int seekRowY = transportY + 24;
        long[] deltas = {-30, -10, -5, 5, 10, 30};
        int sw = 48;
        for (int i = 0; i < deltas.length; i++) {
            final long d = deltas[i];
            String lbl = (d > 0 ? "+" : "") + d + "s";
            addRenderableWidget(Button.builder(Component.literal(lbl), b -> client.seekDelta(d))
                    .bounds(bodyX + i * (sw + 4), seekRowY, sw, 18).build());
        }
        addRenderableWidget(Button.builder(Component.literal(st.muted ? "🔇 Unmute" : "🔊 Mute"), b -> client.setMute(null))
                .bounds(bodyX + 6 * (sw + 4) + 6, seekRowY, 72, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Vol -"), b -> client.volumeDelta(-5))
                .bounds(bodyX + 6 * (sw + 4) + 82, seekRowY, 44, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Vol +"), b -> client.volumeDelta(5))
                .bounds(bodyX + 6 * (sw + 4) + 130, seekRowY, 44, 18).build());
    }

    private void buildDspWidgets() {
        TntronStatus st = client.getStatus();
        int leftX = panelX + 12;
        int colW = (panelW - 36) / 2;
        int rightX = leftX + colW + 12;
        int y = panelY + 62;

        // Volume Slider + Mute
        double vol01 = Math.max(0.0, Math.min(1.0, st.volume / 100.0));
        addRenderableWidget(new AbstractSliderButton(leftX, y, colW - 60, 20,
                Component.literal("Volume: " + Math.round(st.volume) + "%"), vol01) {
            @Override
            protected void updateMessage() {
                setMessage(Component.literal("Volume: " + Math.round(this.value * 100.0) + "%"));
            }

            @Override
            protected void applyValue() {
                client.setVolume(Math.round(this.value * 100.0));
            }
        });
        addRenderableWidget(Button.builder(Component.literal(st.muted ? "Muted" : "Mute"), b -> client.setMute(null))
                .bounds(leftX + colW - 56, y, 56, 20).build());

        // Speed Slider (0.25x .. 3.00x)
        double spd01 = Math.max(0.0, Math.min(1.0, (st.speed - 0.25) / 2.75));
        addRenderableWidget(new AbstractSliderButton(rightX, y, colW, 20,
                Component.literal(String.format("Speed: %.2fx", st.speed)), spd01) {
            @Override
            protected void updateMessage() {
                double s = 0.25 + this.value * 2.75;
                setMessage(Component.literal(String.format("Speed: %.2fx", s)));
            }

            @Override
            protected void applyValue() {
                double s = Math.round((0.25 + this.value * 2.75) * 20.0) / 20.0;
                client.setSpeed(s);
            }
        });

        // Speed Quick Presets
        int py = y + 24;
        double[] speeds = {0.75, 0.85, 1.0, 1.25, 1.5, 2.0};
        int spW = (colW - 20) / 6;
        for (int i = 0; i < speeds.length; i++) {
            final double s = speeds[i];
            addRenderableWidget(Button.builder(Component.literal(s + "x"), b -> client.setSpeed(s))
                    .bounds(rightX + i * (spW + 4), py, spW, 18).build());
        }

        // Pitch Controls on Left
        addRenderableWidget(Button.builder(Component.literal("-1 st"), b -> client.setPitch(null, -1.0, null, null))
                .bounds(leftX, py, 44, 18).build());
        addRenderableWidget(Button.builder(Component.literal("0 st"), b -> client.setPitch(0.0, null, null, null))
                .bounds(leftX + 48, py, 38, 18).build());
        addRenderableWidget(Button.builder(Component.literal("+1 st"), b -> client.setPitch(null, 1.0, null, null))
                .bounds(leftX + 90, py, 44, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Link:" + (st.link_pitch ? "ON" : "OFF")),
                        b -> client.setPitch(null, null, !st.link_pitch, null))
                .bounds(leftX + 138, py, 56, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Form:" + (st.preserve_formant ? "ON" : "OFF")),
                        b -> client.setPitch(null, null, null, !st.preserve_formant))
                .bounds(leftX + 198, py, 54, 18).build());

        // DSP Presets Row
        int dspY = py + 34;
        String[][] presets = {
                {"flat", "Flat"}, {"bass", "Bass+"}, {"vocal", "Vocal"},
                {"warm", "Warm"}, {"rock", "Rock"}, {"8d", "8D Spatial"}
        };
        int pBtnW = (panelW - 24 - 20) / 6;
        for (int i = 0; i < presets.length; i++) {
            final String key = presets[i][0];
            String label = presets[i][1];
            addRenderableWidget(Button.builder(Component.literal(label), b -> client.setDspPreset(key))
                    .bounds(leftX + i * (pBtnW + 4), dspY, pBtnW, 20).build());
        }

        // 3-Band EQ + Spatial Hz
        int eqY = dspY + 38;
        addRenderableWidget(Button.builder(Component.literal("Bass -"), b -> client.stepEq("bass", -1))
                .bounds(leftX, eqY, 54, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Bass +"), b -> client.stepEq("bass", 1))
                .bounds(leftX + 58, eqY, 54, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Mid -"), b -> client.stepEq("mid", -1))
                .bounds(leftX + 124, eqY, 50, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Mid +"), b -> client.stepEq("mid", 1))
                .bounds(leftX + 178, eqY, 50, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Treb -"), b -> client.stepEq("treble", -1))
                .bounds(leftX + 240, eqY, 52, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Treb +"), b -> client.stepEq("treble", 1))
                .bounds(leftX + 296, eqY, 52, 18).build());

        addRenderableWidget(Button.builder(Component.literal("8D: " + (st.spatial_on ? "ON" : "OFF")), b -> client.toggleSpatial())
                .bounds(leftX + 360, eqY, 64, 18).build());
        addRenderableWidget(Button.builder(Component.literal("-Hz"), b -> client.stepSpatialHz(-0.02f))
                .bounds(leftX + 428, eqY, 42, 18).build());
        addRenderableWidget(Button.builder(Component.literal("+Hz"), b -> client.stepSpatialHz(0.02f))
                .bounds(leftX + 474, eqY, 42, 18).build());

        // A-B Section Loop
        int abY = eqY + 38;
        String[] abActions = {"mark_a", "mark_b", "clear", "save", "load"};
        String[] abLabels = {"Mark A", "Mark B", "Clear A-B", "Save A-B", "Load A-B"};
        for (int i = 0; i < abActions.length; i++) {
            final String act = abActions[i];
            addRenderableWidget(Button.builder(Component.literal(abLabels[i]), b -> client.abLoop(act))
                    .bounds(leftX + i * 76, abY, 72, 18).build());
        }

        // Sleep Timer
        int slpY = abY + 38;
        addRenderableWidget(Button.builder(Component.literal("Off"), b -> client.setSleepMinutes(null))
                .bounds(leftX, slpY, 48, 18).build());
        addRenderableWidget(Button.builder(Component.literal("15m"), b -> client.setSleepMinutes(15))
                .bounds(leftX + 52, slpY, 48, 18).build());
        addRenderableWidget(Button.builder(Component.literal("30m"), b -> client.setSleepMinutes(30))
                .bounds(leftX + 104, slpY, 48, 18).build());
        addRenderableWidget(Button.builder(Component.literal("60m"), b -> client.setSleepMinutes(60))
                .bounds(leftX + 156, slpY, 48, 18).build());
        addRenderableWidget(Button.builder(Component.literal("-5m"), b -> client.stepSleepMinutes(-5))
                .bounds(leftX + 208, slpY, 48, 18).build());
        addRenderableWidget(Button.builder(Component.literal("+5m"), b -> client.stepSleepMinutes(5))
                .bounds(leftX + 260, slpY, 48, 18).build());
    }

    private void buildLibraryWidgets() {
        int leftX = panelX + 12;
        int topY = panelY + 58;

        // Sub-tabs: Tracks | Folders | Queue
        LibrarySubTab[] subs = LibrarySubTab.values();
        String[] subLbls = {"Tracks", "Folders", "Queue & Upcoming"};
        for (int i = 0; i < subs.length; i++) {
            final LibrarySubTab s = subs[i];
            addRenderableWidget(Button.builder(Component.literal((s == libSubTab ? "▸ " : "") + subLbls[i]), b -> {
                libSubTab = s;
                scrollOffset = 0;
                rebuildWidgets();
            }).bounds(leftX + i * 112, topY, 108, 18).build());
        }

        addRenderableWidget(Button.builder(Component.literal("♥ Favs"), b -> client.playFavorites(false))
                .bounds(panelX + panelW - 186, topY, 56, 18).build());
        addRenderableWidget(Button.builder(Component.literal("▶ All"), b -> client.playAllLibrary(true))
                .bounds(panelX + panelW - 126, topY, 52, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Rescan"), b -> {
            client.rescan();
            client.queryLibrary(searchQuery, selectedFolder, 120, lib -> rebuildWidgets());
        }).bounds(panelX + panelW - 70, topY, 58, 18).build());

        // Search bar
        int searchY = topY + 22;
        searchBox = new EditBox(this.font, leftX, searchY, panelW - 160, 18, Component.literal("Search library"));
        searchBox.setValue(searchQuery);
        searchBox.setHint(Component.literal("Search tracks, artists, albums..."));
        searchBox.setResponder(v -> searchQuery = v);
        addRenderableWidget(searchBox);

        addRenderableWidget(Button.builder(Component.literal("Search"), b -> {
            scrollOffset = 0;
            libSubTab = LibrarySubTab.TRACKS;
            client.queryLibrary(searchQuery, selectedFolder, 120, lib -> rebuildWidgets());
        }).bounds(panelX + panelW - 144, searchY, 64, 18).build());
        addRenderableWidget(Button.builder(Component.literal("Clear"), b -> {
            searchQuery = "";
            selectedFolder = "";
            scrollOffset = 0;
            client.queryLibrary("", "", 120, lib -> rebuildWidgets());
        }).bounds(panelX + panelW - 76, searchY, 64, 18).build());

        // Rows area
        int listTop = searchY + 24;
        int rowH = 22;
        int maxRows = Math.max(4, (panelY + panelH - listTop - 12) / rowH);
        LibraryData lib = client.getCachedLibrary();
        TntronStatus st = client.getStatus();

        if (libSubTab == LibrarySubTab.TRACKS) {
            List<LibraryTrack> tracks = lib.tracks;
            int maxScroll = Math.max(0, tracks.size() - maxRows);
            scrollOffset = Math.max(0, Math.min(scrollOffset, maxScroll));
            for (int i = 0; i < maxRows && (i + scrollOffset) < tracks.size(); i++) {
                LibraryTrack tr = tracks.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                addRenderableWidget(Button.builder(Component.literal("▶"), b -> client.playPath(tr.path))
                        .bounds(panelX + panelW - 120, ry, 24, 18).build());
                addRenderableWidget(Button.builder(Component.literal("+Q"), b -> client.addToQueue(tr.path))
                        .bounds(panelX + panelW - 94, ry, 26, 18).build());
                addRenderableWidget(Button.builder(Component.literal("Next"), b -> client.playNextInQueue(tr.path))
                        .bounds(panelX + panelW - 66, ry, 34, 18).build());
                addRenderableWidget(Button.builder(Component.literal(tr.is_favorite ? "♥" : "♡"), b -> {
                    client.toggleFavorite(tr.path);
                    client.queryLibrary(searchQuery, selectedFolder, 120, l -> rebuildWidgets());
                }).bounds(panelX + panelW - 30, ry, 18, 18).build());
            }
        } else if (libSubTab == LibrarySubTab.FOLDERS) {
            List<LibraryFolder> folders = lib.folders;
            int maxScroll = Math.max(0, folders.size() - maxRows);
            scrollOffset = Math.max(0, Math.min(scrollOffset, maxScroll));
            for (int i = 0; i < maxRows && (i + scrollOffset) < folders.size(); i++) {
                LibraryFolder f = folders.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                addRenderableWidget(Button.builder(Component.literal("Open"), b -> {
                    selectedFolder = f.path;
                    libSubTab = LibrarySubTab.TRACKS;
                    scrollOffset = 0;
                    client.queryLibrary(searchQuery, selectedFolder, 120, l -> rebuildWidgets());
                }).bounds(panelX + panelW - 144, ry, 42, 18).build());
                addRenderableWidget(Button.builder(Component.literal("▶ Play"), b -> client.playFolder(f.path, false))
                        .bounds(panelX + panelW - 98, ry, 46, 18).build());
                addRenderableWidget(Button.builder(Component.literal("🔀"), b -> client.playFolder(f.path, true))
                        .bounds(panelX + panelW - 50, ry, 20, 18).build());
                addRenderableWidget(Button.builder(Component.literal("+Q"), b -> client.addFolderToQueue(f.path))
                        .bounds(panelX + panelW - 28, ry, 20, 18).build());
            }
        } else {
            addRenderableWidget(Button.builder(Component.literal("Clear Queue"), b -> client.clearQueue())
                    .bounds(panelX + panelW - 96, searchY, 84, 18).build());
            List<PopupTrackItem> combined = buildCombinedQueueList(st);
            int maxScroll = Math.max(0, combined.size() - maxRows);
            scrollOffset = Math.max(0, Math.min(scrollOffset, maxScroll));
            for (int i = 0; i < maxRows && (i + scrollOffset) < combined.size(); i++) {
                PopupTrackItem item = combined.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                int idx = item.playlist_index != null ? item.playlist_index : i;
                if (item.is_queued) {
                    addRenderableWidget(Button.builder(Component.literal("▶ Play"), b -> client.playQueueItem(idx))
                            .bounds(panelX + panelW - 92, ry, 48, 18).build());
                    addRenderableWidget(Button.builder(Component.literal("✕"), b -> client.removeFromQueue(idx))
                            .bounds(panelX + panelW - 40, ry, 28, 18).build());
                } else {
                    addRenderableWidget(Button.builder(Component.literal("▶ Jump"), b -> client.playUpcoming(idx))
                            .bounds(panelX + panelW - 68, ry, 56, 18).build());
                }
            }
        }
    }

    private List<PopupTrackItem> buildCombinedQueueList(TntronStatus st) {
        List<PopupTrackItem> out = new ArrayList<>();
        if (st.queue_items != null) out.addAll(st.queue_items);
        if (st.playlist_items != null) out.addAll(st.playlist_items);
        return out;
    }

    private void buildThemesConfigWidgets() {
        int leftX = panelX + 12;
        int topY = panelY + 58;

        ConfigSubTab[] subs = ConfigSubTab.values();
        String[] subLbls = {"tntron Themes", "tntron Extensions", "Mod & HUD Settings"};
        for (int i = 0; i < subs.length; i++) {
            final ConfigSubTab s = subs[i];
            addRenderableWidget(Button.builder(Component.literal((s == cfgSubTab ? "▸ " : "") + subLbls[i]), b -> {
                cfgSubTab = s;
                scrollOffset = 0;
                rebuildWidgets();
            }).bounds(leftX + i * 134, topY, 130, 18).build());
        }

        TntronStatus st = client.getStatus();
        addRenderableWidget(Button.builder(Component.literal(st.hide_ui ? "Show UI" : "Hide UI"), b -> client.toggleHideUi())
                .bounds(panelX + panelW - 76, topY, 64, 18).build());

        int areaY = topY + 26;
        if (cfgSubTab == ConfigSubTab.THEMES) {
            List<ThemeItem> themes = client.getCachedThemes();
            int cols = 3;
            int btnW = (panelW - 24 - (cols - 1) * 6) / cols;
            int rowH = 24;
            int maxRows = Math.max(4, (panelY + panelH - areaY - 10) / rowH);
            int totalRows = (themes.size() + cols - 1) / cols;
            scrollOffset = Math.max(0, Math.min(scrollOffset, Math.max(0, totalRows - maxRows)));

            for (int r = 0; r < maxRows; r++) {
                int rowIdx = r + scrollOffset;
                for (int c = 0; c < cols; c++) {
                    int idx = rowIdx * cols + c;
                    if (idx >= themes.size()) break;
                    ThemeItem th = themes.get(idx);
                    boolean active = th.slug.equalsIgnoreCase(st.active_theme);
                    String lbl = (active ? "✓ " : "") + th.label;
                    addRenderableWidget(Button.builder(Component.literal(lbl), b -> client.setTheme(th.slug))
                            .bounds(leftX + c * (btnW + 6), areaY + r * rowH, btnW, 20).build());
                }
            }
        } else if (cfgSubTab == ConfigSubTab.EXTENSIONS) {
            List<ExtensionItem> exts = client.getCachedExtensions();
            int rowH = 22;
            int maxRows = Math.max(4, (panelY + panelH - areaY - 10) / rowH);
            scrollOffset = Math.max(0, Math.min(scrollOffset, Math.max(0, exts.size() - maxRows)));
            for (int i = 0; i < maxRows && (i + scrollOffset) < exts.size(); i++) {
                ExtensionItem ext = exts.get(i + scrollOffset);
                int ry = areaY + i * rowH;
                String lbl = ext.enabled ? "ON" : "OFF";
                addRenderableWidget(Button.builder(Component.literal(lbl), b -> client.toggleExtension(ext.id))
                        .bounds(panelX + panelW - 56, ry, 44, 18).build());
            }
        } else {
            int colW = (panelW - 36) / 2;
            int rx = leftX + colW + 12;
            int y = areaY;

            // Left Column: HUD & Visuals
            addRenderableWidget(Button.builder(Component.literal("HUD Overlay: " + (config.hudEnabled ? "ON" : "OFF")), b -> {
                config.hudEnabled = !config.hudEnabled;
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("HUD Corner: " + config.hudPosition), b -> {
                config.hudPosition = switch (config.hudPosition) {
                    case "top_left" -> "top_right";
                    case "top_right" -> "bottom_right";
                    case "bottom_right" -> "bottom_left";
                    default -> "top_left";
                };
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 24, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("HUD Cover: " + (config.showCover ? "ON" : "OFF")), b -> {
                config.showCover = !config.showCover;
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 48, colW / 2 - 2, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Spectrum: " + (config.showSpectrum ? "ON" : "OFF")), b -> {
                config.showSpectrum = !config.showSpectrum;
                config.save();
                rebuildWidgets();
            }).bounds(leftX + colW / 2 + 2, y + 48, colW / 2 - 2, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Lyrics Bar: " + (config.lyricsOverlayEnabled ? "ON" : "OFF")), b -> {
                config.lyricsOverlayEnabled = !config.lyricsOverlayEnabled;
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 72, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Beat Vignette: " + (config.beatVignette ? "ON" : "OFF")), b -> {
                config.beatVignette = !config.beatVignette;
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 96, colW / 2 - 2, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Beat Particles: " + (config.beatParticles ? "ON" : "OFF")), b -> {
                config.beatParticles = !config.beatParticles;
                config.save();
                rebuildWidgets();
            }).bounds(leftX + colW / 2 + 2, y + 96, colW / 2 - 2, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Beat FOV Pulse: " + (config.beatFovPulse ? "ON" : "OFF")), b -> {
                config.beatFovPulse = !config.beatFovPulse;
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 120, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("HUD Scale -"), b -> {
                config.hudScale = Math.max(0.6f, config.hudScale - 0.1f);
                config.save();
                rebuildWidgets();
            }).bounds(leftX, y + 144, colW / 2 - 2, 20).build());
            addRenderableWidget(Button.builder(Component.literal("HUD Scale +"), b -> {
                config.hudScale = Math.min(1.6f, config.hudScale + 0.1f);
                config.save();
                rebuildWidgets();
            }).bounds(leftX + colW / 2 + 2, y + 144, colW / 2 - 2, 20).build());

            // Right Column: Audio & Connection
            addRenderableWidget(Button.builder(Component.literal("Auto-Mute MC Music: " + (config.autoMuteMcMusic ? "ON" : "OFF")), b -> {
                config.autoMuteMcMusic = !config.autoMuteMcMusic;
                config.save();
                rebuildWidgets();
            }).bounds(rx, y, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Track Notify: " + config.notifyMode), b -> {
                config.notifyMode = switch (config.notifyMode) {
                    case "toast" -> "actionbar";
                    case "actionbar" -> "chat";
                    case "chat" -> "off";
                    default -> "toast";
                };
                config.save();
                rebuildWidgets();
            }).bounds(rx, y + 24, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Send MC Telemetry: " + (config.sendTelemetry ? "ON" : "OFF")), b -> {
                config.sendTelemetry = !config.sendTelemetry;
                config.save();
                rebuildWidgets();
            }).bounds(rx, y + 48, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Sync Theme Accent: " + (config.syncAccentWithTheme ? "ON" : "OFF")), b -> {
                config.syncAccentWithTheme = !config.syncAccentWithTheme;
                config.save();
                rebuildWidgets();
            }).bounds(rx, y + 72, colW, 20).build());

            addRenderableWidget(Button.builder(Component.literal("Transport: " + config.connectionMode.toUpperCase()), b -> {
                config.connectionMode = switch (config.connectionMode) {
                    case "auto" -> "tcp";
                    case "tcp" -> "unix";
                    default -> "auto";
                };
                config.save();
                client.reconnect();
                rebuildWidgets();
            }).bounds(rx, y + 96, colW, 20).build());

            portBox = new EditBox(this.font, rx, y + 120, colW - 76, 20, Component.literal("TCP Port"));
            portBox.setValue(String.valueOf(config.tcpPort));
            addRenderableWidget(portBox);

            addRenderableWidget(Button.builder(Component.literal("Apply Port"), b -> {
                if (portBox != null) {
                    try {
                        int p = Integer.parseInt(portBox.getValue().trim());
                        if (p >= 1024 && p <= 65535) {
                            config.tcpPort = p;
                            config.save();
                            client.reconnect();
                        }
                    } catch (NumberFormatException ignored) {
                    }
                }
                rebuildWidgets();
            }).bounds(rx + colW - 72, y + 120, 72, 20).build());
        }
    }

    @Override
    public void tick() {
        super.tick();
        TntronStatus st = client.getStatus();
        int h = (st.state + "|" + st.track_path + "|" + st.shuffle + "|" + st.loop_mode + "|"
                + st.muted + "|" + st.is_favorite + "|" + st.spatial_on + "|" + st.link_pitch
                + "|" + st.preserve_formant + "|" + st.active_theme + "|" + st.hide_ui + "|" + client.isConnected()).hashCode();
        if (h != lastStatusHash) {
            lastStatusHash = h;
            boolean typing = (searchBox != null && searchBox.isFocused()) || (portBox != null && portBox.isFocused());
            if (!typing && activeTab != Tab.DSP_AUDIO) {
                rebuildWidgets();
            }
        }
    }

    @Override
    public void extractBackground(GuiGraphicsExtractor graphics, int mouseX, int mouseY, float partialTick) {
        graphics.fill(0, 0, this.width, this.height, 0x88000000);
        TntronStatus st = client.getStatus();
        int accent = TntronThemeColors.resolveAccent(config, st.active_theme, client.getCachedThemes());
        drawPanelBackground(graphics, accent);
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor g, int mouseX, int mouseY, float partialTick) {
        super.extractRenderState(g, mouseX, mouseY, partialTick);
        drawHeaderAndTabContents(g, mouseX, mouseY);
    }

    // Draw panel surface in extractBackground before widgets, and dynamic content on top via a helper!
    // Wait: let's override extractRenderState cleanly: draw panel background -> call super.extractRenderState -> draw dynamic text/bars/visualizers!
    // Note: Screen.extractRenderState calls extractBackground first, then Renderables! So putting panel background in extractBackground and overlays after super.extractRenderState works cleanly without covering buttons!

    private void drawPanelBackground(GuiGraphicsExtractor g, int accent) {
        g.fill(panelX, panelY, panelX + panelW, panelY + panelH, 0xEE0D1017);
        g.fill(panelX, panelY, panelX + panelW, panelY + 2, accent);
        g.fill(panelX, panelY + panelH - 1, panelX + panelW, panelY + panelH, 0x44FFFFFF);
        g.fill(panelX, panelY, panelX + 1, panelY + panelH, 0x33FFFFFF);
        g.fill(panelX + panelW - 1, panelY, panelX + panelW, panelY + panelH, 0x33FFFFFF);
        g.fill(panelX + 1, panelY + 28, panelX + panelW - 1, panelY + 29, 0x22FFFFFF);
    }

    // Override extractBackground to draw backdrop + panel frame, then super.extractRenderState draws widgets, then we draw foreground overlays!
    // Wait: we defined extractBackground above; let's merge drawPanelBackground into extractBackground and draw foreground in extractRenderState!
    private void drawHeaderAndTabContents(GuiGraphicsExtractor g, int mouseX, int mouseY) {
        TntronStatus st = client.getStatus();
        boolean conn = client.isConnected();
        int accent = TntronThemeColors.resolveAccent(config, st.active_theme, client.getCachedThemes());

        // Header brand + connection pills
        g.text(this.font, "tntron", panelX + 14, panelY + 12, accent, true);
        g.text(this.font, "· Fabric 26.2", panelX + 52, panelY + 12, 0xFF94A3B8, false);

        String connLabel = conn ? ("● " + client.getTransportLabel()) : "○ Offline";
        int connColor = conn ? 0xFF22C55E : 0xFFEF4444;
        g.text(this.font, connLabel, panelX + 135, panelY + 12, connColor, false);

        String extLbl = st.mc_link_enabled ? "MC Ext: Active" : "MC Ext: Enable in tntron";
        int extCol = st.mc_link_enabled ? accent : 0xFFF59E0B;
        g.text(this.font, extLbl, panelX + 275, panelY + 12, extCol, false);

        switch (activeTab) {
            case NOW_PLAYING -> renderNowPlayingDetails(g, st, accent, mouseX, mouseY);
            case DSP_AUDIO -> renderDspDetails(g, st, accent);
            case LIBRARY_QUEUE -> renderLibraryDetails(g, st, accent);
            case THEMES_CONFIG -> renderThemesConfigDetails(g, st, accent);
        }
    }

    private void renderNowPlayingDetails(GuiGraphicsExtractor g, TntronStatus st, int accent, int mouseX, int mouseY) {
        int artX = panelX + 14;
        int artY = panelY + 62;
        int artSize = 76;

        g.fill(artX - 1, artY - 1, artX + artSize + 1, artY + artSize + 1, 0xFF1E293B);
        if (coverArt.hasCover()) {
            g.blit(RenderPipelines.GUI_TEXTURED, CoverArtManager.COVER_TEX_ID, artX, artY, 0f, 0f, artSize, artSize, artSize, artSize);
        } else {
            g.fill(artX, artY, artX + artSize, artY + artSize, 0xFF18181B);
            g.fill(artX + 12, artY + 12, artX + artSize - 12, artY + artSize - 12, 0xFF27272A);
            g.fill(artX + 28, artY + 28, artX + artSize - 28, artY + artSize - 28, accent);
            g.fill(artX + 35, artY + 35, artX + artSize - 35, artY + artSize - 35, 0xFF09090B);
        }

        int infoX = artX + artSize + 14;
        int maxW = panelX + panelW - infoX - 16;

        String title = st.title != null && !st.title.isEmpty() ? st.title : "No Active Track";
        String artist = st.artist != null && !st.artist.isEmpty() ? st.artist : "Play a track in tntron or browse Library";
        String album = st.album != null ? st.album : "";

        g.text(this.font, font.plainSubstrByWidth(title, maxW), infoX, artY + 4, 0xFFF8FAFC, true);
        g.text(this.font, font.plainSubstrByWidth(artist, maxW), infoX, artY + 18, 0xFFCBD5E1, false);
        if (!album.isEmpty()) {
            g.text(this.font, font.plainSubstrByWidth(album, maxW), infoX, artY + 30, 0xFF94A3B8, false);
        }

        StringBuilder badges = new StringBuilder();
        if (st.codec != null && !st.codec.isBlank()) badges.append(st.codec.toUpperCase());
        if (st.bitrate_kbps != null) badges.append(" · ").append(st.bitrate_kbps).append(" kbps");
        if (st.sample_rate_hz != null) badges.append(" · ").append(st.sample_rate_hz / 1000).append("kHz");
        if (st.bpm != null && st.bpm > 0) badges.append(" · ").append(Math.round(st.bpm)).append(" BPM");
        badges.append(String.format(" · Vol %.0f%% · %.2fx", st.volume, st.speed));
        g.text(this.font, font.plainSubstrByWidth(badges.toString(), maxW), infoX, artY + 46, accent, false);

        // Interactive Click-to-Seek Bar
        seekBarX = infoX;
        seekBarY = artY + 64;
        seekBarW = maxW;
        seekBarH = 8;

        g.fill(seekBarX, seekBarY, seekBarX + seekBarW, seekBarY + seekBarH, 0xFF1E293B);
        double frac = st.duration > 0 ? Math.max(0.0, Math.min(1.0, st.position / st.duration)) : 0.0;
        int fillW = (int) Math.round(seekBarW * frac);
        if (fillW > 0) {
            g.fill(seekBarX, seekBarY, seekBarX + fillW, seekBarY + seekBarH, accent);
        }
        // A-B markers
        if (st.duration > 0) {
            if (st.ab_loop_a != null) {
                int ax = seekBarX + (int) Math.round(seekBarW * Math.max(0, Math.min(1, st.ab_loop_a / st.duration)));
                g.fill(ax, seekBarY - 2, ax + 2, seekBarY + seekBarH + 2, 0xFF38BDF8);
            }
            if (st.ab_loop_b != null) {
                int bx = seekBarX + (int) Math.round(seekBarW * Math.max(0, Math.min(1, st.ab_loop_b / st.duration)));
                g.fill(bx, seekBarY - 2, bx + 2, seekBarY + seekBarH + 2, 0xFFF59E0B);
            }
        }

        String timeStr = TntronThemeColors.formatTime(st.position) + " / " + TntronThemeColors.formatTime(st.duration);
        g.text(this.font, timeStr, seekBarX, seekBarY + 11, 0xFFCBD5E1, false);
        if (mouseX >= seekBarX && mouseX <= seekBarX + seekBarW && mouseY >= seekBarY - 3 && mouseY <= seekBarY + seekBarH + 3 && st.duration > 0) {
            double hoverFrac = (mouseX - seekBarX) / (double) seekBarW;
            String hoverTime = "Click to seek -> " + TntronThemeColors.formatTime(hoverFrac * st.duration);
            g.text(this.font, hoverTime, seekBarX + seekBarW - font.width(hoverTime), seekBarY + 11, 0xFF94A3B8, false);
        }

        // Bottom Half: Spectrum Visualizer (left) + Synced Lyrics Window (right)
        int botY = panelY + 206;
        int botH = panelY + panelH - botY - 12;
        int specW = (panelW - 36) / 2;

        g.fill(panelX + 12, botY, panelX + 12 + specW, botY + botH, 0xAA080A0F);
        g.text(this.font, "Live Spectrum", panelX + 18, botY + 6, 0xFF94A3B8, false);
        int barAreaX = panelX + 18;
        int barAreaW = specW - 12;
        int barBottom = botY + botH - 8;
        int maxBarH = Math.max(16, botH - 26);
        int bw = (barAreaW - 15 * 3) / 16;
        for (int i = 0; i < 16; i++) {
            float v = (st.bands != null && i < st.bands.length) ? Math.max(0f, Math.min(1f, st.bands[i])) : 0f;
            int bh = Math.max(2, Math.round(v * maxBarH));
            int bx = barAreaX + i * (bw + 3);
            g.fill(bx, barBottom - bh, bx + bw, barBottom, TntronThemeColors.blendRgb(accent, 0xFFFFFFFF, v * 0.4f));
        }

        int lyrX = panelX + 24 + specW;
        g.fill(lyrX, botY, lyrX + specW, botY + botH, 0xAA080A0F);
        g.text(this.font, "Synced Lyrics", lyrX + 8, botY + 6, 0xFF94A3B8, false);
        if (st.lyrics_window != null && !st.lyrics_window.isEmpty()) {
            int ly = botY + 22;
            for (String line : st.lyrics_window) {
                if (ly + 11 > botY + botH - 4) break;
                boolean isCurr = st.lyric != null && st.lyric.equals(line);
                String draw = font.plainSubstrByWidth(line, specW - 16);
                g.text(this.font, draw, lyrX + 8, ly, isCurr ? accent : 0xFFCBD5E1, isCurr);
                ly += 13;
            }
        } else if (st.lyric != null && !st.lyric.isEmpty()) {
            g.text(this.font, font.plainSubstrByWidth(st.lyric, specW - 16), lyrX + 8, botY + 26, accent, true);
        } else {
            g.text(this.font, "No synced lyrics for current track", lyrX + 8, botY + 26, 0xFF64748B, false);
        }
    }

    private void renderDspDetails(GuiGraphicsExtractor g, TntronStatus st, int accent) {
        int leftX = panelX + 12;
        g.text(this.font, String.format("Pitch: %+.1f semitones", st.pitch_st), leftX, panelY + 108, 0xFFCBD5E1, false);
        g.text(this.font, "Active DSP Preset: " + st.dsp_preset, leftX, panelY + 126, accent, true);
        g.text(this.font, String.format("3-Band EQ: Bass %+d dB | Mid %+d dB | Treble %+d dB  ·  Spatial %.2f Hz",
                st.bass_db, st.mid_db, st.treble_db, st.spatial_hz), leftX, panelY + 166, 0xFFCBD5E1, false);

        String abTxt = "A-B Loop: A=" + (st.ab_loop_a != null ? TntronThemeColors.formatTime(st.ab_loop_a) : "--")
                + "  B=" + (st.ab_loop_b != null ? TntronThemeColors.formatTime(st.ab_loop_b) : "--");
        g.text(this.font, abTxt, leftX, panelY + 204, 0xFFCBD5E1, false);

        String slpTxt = st.sleep_remaining_sec != null
                ? ("Sleep Timer: " + TntronThemeColors.formatTime(st.sleep_remaining_sec) + " remaining")
                : "Sleep Timer: Inactive";
        g.text(this.font, slpTxt, leftX, panelY + 242, st.sleep_remaining_sec != null ? accent : 0xFFCBD5E1, false);
    }

    private void renderLibraryDetails(GuiGraphicsExtractor g, TntronStatus st, int accent) {
        int leftX = panelX + 14;
        int listTop = panelY + 104;
        int rowH = 22;
        int maxRows = Math.max(4, (panelY + panelH - listTop - 12) / rowH);
        LibraryData lib = client.getCachedLibrary();

        if (libSubTab == LibrarySubTab.TRACKS) {
            List<LibraryTrack> tracks = lib.tracks;
            if (tracks.isEmpty()) {
                g.text(this.font, "No tracks loaded. Click Search or Rescan.", leftX, listTop + 8, 0xFF94A3B8, false);
                return;
            }
            for (int i = 0; i < maxRows && (i + scrollOffset) < tracks.size(); i++) {
                LibraryTrack tr = tracks.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                g.fill(leftX - 2, ry - 1, panelX + panelW - 12, ry + 19, (i % 2 == 0) ? 0x55181C26 : 0x3311141C);
                boolean isCurr = tr.path.equals(st.track_path);
                String label = tr.title + (tr.artist.isEmpty() ? "" : (" — " + tr.artist));
                String dur = TntronThemeColors.formatTime(tr.duration);
                int maxW = panelW - 180;
                g.text(this.font, font.plainSubstrByWidth(label, maxW), leftX + 2, ry + 5, isCurr ? accent : 0xFFF1F5F9, isCurr);
                g.text(this.font, dur, panelX + panelW - 154, ry + 5, 0xFF94A3B8, false);
            }
        } else if (libSubTab == LibrarySubTab.FOLDERS) {
            List<LibraryFolder> folders = lib.folders;
            if (folders.isEmpty()) {
                g.text(this.font, "No folders/playlists found.", leftX, listTop + 8, 0xFF94A3B8, false);
                return;
            }
            for (int i = 0; i < maxRows && (i + scrollOffset) < folders.size(); i++) {
                LibraryFolder f = folders.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                g.fill(leftX - 2, ry - 1, panelX + panelW - 12, ry + 19, (i % 2 == 0) ? 0x55181C26 : 0x3311141C);
                String lbl = "📁 " + f.name + " (" + f.track_count + " tracks)";
                g.text(this.font, font.plainSubstrByWidth(lbl, panelW - 170), leftX + 2, ry + 5, 0xFFF1F5F9, false);
            }
        } else {
            List<PopupTrackItem> combined = buildCombinedQueueList(st);
            if (combined.isEmpty()) {
                g.text(this.font, "Queue & upcoming playlist are empty.", leftX, listTop + 8, 0xFF94A3B8, false);
                return;
            }
            for (int i = 0; i < maxRows && (i + scrollOffset) < combined.size(); i++) {
                PopupTrackItem item = combined.get(i + scrollOffset);
                int ry = listTop + i * rowH;
                g.fill(leftX - 2, ry - 1, panelX + panelW - 12, ry + 19, (i % 2 == 0) ? 0x55181C26 : 0x3311141C);
                String prefix = item.is_queued ? "[Q] " : (item.is_current ? "▶ " : "   ");
                String lbl = prefix + item.title + (item.artist.isEmpty() ? "" : (" — " + item.artist));
                g.text(this.font, font.plainSubstrByWidth(lbl, panelW - 130), leftX + 2, ry + 5,
                        item.is_current ? accent : (item.is_queued ? 0xFF38BDF8 : 0xFFF1F5F9), item.is_current);
            }
        }
    }

    private void renderThemesConfigDetails(GuiGraphicsExtractor g, TntronStatus st, int accent) {
        int leftX = panelX + 14;
        int areaY = panelY + 84;
        if (cfgSubTab == ConfigSubTab.EXTENSIONS) {
            List<ExtensionItem> exts = client.getCachedExtensions();
            int rowH = 22;
            int maxRows = Math.max(4, (panelY + panelH - areaY - 10) / rowH);
            for (int i = 0; i < maxRows && (i + scrollOffset) < exts.size(); i++) {
                ExtensionItem ext = exts.get(i + scrollOffset);
                int ry = areaY + i * rowH;
                g.fill(leftX - 2, ry - 1, panelX + panelW - 12, ry + 19, (i % 2 == 0) ? 0x55181C26 : 0x3311141C);
                String lbl = ext.name + " v" + ext.version;
                g.text(this.font, font.plainSubstrByWidth(lbl, 190), leftX + 2, ry + 5, ext.enabled ? accent : 0xFFCBD5E1, ext.enabled);
                g.text(this.font, font.plainSubstrByWidth(ext.description, panelW - 275), leftX + 200, ry + 5, 0xFF94A3B8, false);
            }
        } else if (cfgSubTab == ConfigSubTab.MOD_SETTINGS) {
            String hint = String.format("HUD Scale: %.1fx  ·  TCP Target: %s:%d  ·  Status: %s",
                    config.hudScale, config.tcpHost, config.tcpPort,
                    client.isConnected() ? "Connected" : (client.getLastError().isEmpty() ? "Connecting..." : client.getLastError()));
            g.text(this.font, font.plainSubstrByWidth(hint, panelW - 28), leftX, panelY + panelH - 20, 0xFF94A3B8, false);
        }
    }

    @Override
    public boolean mouseClicked(MouseButtonEvent event, boolean doubleClick) {
        if (event.button() == 0 && activeTab == Tab.NOW_PLAYING) {
            double mx = event.x();
            double my = event.y();
            TntronStatus st = client.getStatus();
            if (st.duration > 0 && mx >= seekBarX && mx <= seekBarX + seekBarW && my >= seekBarY - 3 && my <= seekBarY + seekBarH + 3) {
                double frac = Math.max(0.0, Math.min(1.0, (mx - seekBarX) / (double) seekBarW));
                client.seekAbsolute(frac * st.duration);
                return true;
            }
        }
        return super.mouseClicked(event, doubleClick);
    }

    @Override
    public boolean mouseScrolled(double mouseX, double mouseY, double scrollX, double scrollY) {
        if (activeTab == Tab.LIBRARY_QUEUE || (activeTab == Tab.THEMES_CONFIG && cfgSubTab != ConfigSubTab.MOD_SETTINGS)) {
            int delta = scrollY > 0 ? -1 : 1;
            int next = Math.max(0, scrollOffset + delta);
            if (next != scrollOffset) {
                scrollOffset = next;
                rebuildWidgets();
            }
            return true;
        }
        return super.mouseScrolled(mouseX, mouseY, scrollX, scrollY);
    }

    @Override
    public boolean keyPressed(KeyEvent event) {
        boolean typing = (searchBox != null && searchBox.isFocused()) || (portBox != null && portBox.isFocused());
        if (typing && event.key() == GLFW.GLFW_KEY_ENTER && searchBox != null && searchBox.isFocused()) {
            scrollOffset = 0;
            libSubTab = LibrarySubTab.TRACKS;
            client.queryLibrary(searchQuery, selectedFolder, 120, lib -> rebuildWidgets());
            return true;
        }
        if (!typing) {
            if (event.key() == GLFW.GLFW_KEY_SPACE) {
                client.toggle();
                return true;
            } else if (event.key() == GLFW.GLFW_KEY_LEFT) {
                client.seekDelta(-5);
                return true;
            } else if (event.key() == GLFW.GLFW_KEY_RIGHT) {
                client.seekDelta(5);
                return true;
            }
        }
        return super.keyPressed(event);
    }

    // Ensure panel frame renders behind widgets and foreground overlays render cleanly
    {
        // instance initializer unused
    }
}
