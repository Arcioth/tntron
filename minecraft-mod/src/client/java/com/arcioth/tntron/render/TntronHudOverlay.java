package com.arcioth.tntron.render;

import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronClient;
import com.arcioth.tntron.ipc.TntronModels.TntronStatus;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElement;
import net.minecraft.client.DeltaTracker;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.Font;
import net.minecraft.client.gui.GuiGraphicsExtractor;
import net.minecraft.client.renderer.RenderPipelines;

public final class TntronHudOverlay implements HudElement {
    private final TntronClient client;
    private final TntronModConfig config;
    private final CoverArtManager coverArt;

    private final float[] smoothBands = new float[16];
    private float smoothBeat = 0.0f;
    private double interpolatedPos = 0.0;
    private long lastFrameNs = System.nanoTime();

    public TntronHudOverlay(TntronClient client, TntronModConfig config, CoverArtManager coverArt) {
        this.client = client;
        this.config = config;
        this.coverArt = coverArt;
    }

    public float getSmoothBeat() {
        return smoothBeat;
    }

    @Override
    public void extractRenderState(GuiGraphicsExtractor graphics, DeltaTracker deltaTracker) {
        Minecraft mc = Minecraft.getInstance();
        if (mc.gui.hud.isHidden() || mc.gui.screen() != null) {
            return;
        }

        long nowNs = System.nanoTime();
        float dt = Math.min(0.1f, Math.max(0.001f, (nowNs - lastFrameNs) / 1_000_000_000.0f));
        lastFrameNs = nowNs;

        TntronStatus st = client.getStatus();
        boolean connected = client.isConnected();

        // Smooth audio spectrum & beat interpolation
        float targetBeat = (connected && st.isPlaying()) ? Math.max(0f, Math.min(1.5f, st.beat)) : 0f;
        smoothBeat += (targetBeat - smoothBeat) * Math.min(1f, dt * 18f);

        for (int i = 0; i < 16; i++) {
            float target = (connected && st.isPlaying() && st.bands != null && i < st.bands.length)
                    ? Math.max(0f, Math.min(1.0f, st.bands[i]))
                    : 0f;
            float rate = target > smoothBands[i] ? 24f : 10f;
            smoothBands[i] += (target - smoothBands[i]) * Math.min(1f, dt * rate);
        }

        if (connected && st.isPlaying()) {
            double diff = Math.abs(interpolatedPos - st.position);
            if (diff > 1.2) {
                interpolatedPos = st.position;
            } else {
                interpolatedPos += dt * Math.max(0.25, st.speed);
                interpolatedPos = Math.min(st.duration > 0 ? st.duration : interpolatedPos, interpolatedPos);
            }
        } else {
            interpolatedPos = st.position;
        }

        int accent = TntronThemeColors.resolveAccent(config, st.active_theme, client.getCachedThemes());

        // Beat-reactive screen edge vignette glow
        if (config.reactiveEnabled && config.beatVignette && connected && st.isPlaying() && smoothBeat > 0.04f) {
            renderBeatVignette(graphics, accent, smoothBeat * config.beatIntensity);
        }

        if (!config.hudEnabled || !connected) {
            return;
        }

        Font font = mc.font;
        int screenW = graphics.guiWidth();
        int screenH = graphics.guiHeight();

        // Karaoke synced lyrics bar above hotbar
        if (config.lyricsOverlayEnabled && st.isPlaying() && st.lyric != null && !st.lyric.isBlank()) {
            renderLyricsBar(graphics, font, screenW, screenH, st, accent);
        }

        if (!st.hasTrack() && !st.isPaused()) {
            return;
        }

        renderNowPlayingCard(graphics, font, screenW, screenH, st, accent);
    }

    private void renderBeatVignette(GuiGraphicsExtractor g, int accent, float strength) {
        int w = g.guiWidth();
        int h = g.guiHeight();
        int layers = 6;
        float clamped = Math.min(1.0f, strength * 0.45f);
        for (int i = 0; i < layers; i++) {
            float alpha = clamped * (1.0f - (float) i / layers) * 0.35f;
            int col = TntronThemeColors.withAlpha(accent, alpha);
            int pad = i * 2;
            g.fill(pad, pad, w - pad, pad + 2, col);
            g.fill(pad, h - pad - 2, w - pad, h - pad, col);
            g.fill(pad, pad + 2, pad + 2, h - pad - 2, col);
            g.fill(w - pad - 2, pad + 2, w - pad, h - pad - 2, col);
        }
    }

    private void renderLyricsBar(GuiGraphicsExtractor g, Font font, int screenW, int screenH, TntronStatus st, int accent) {
        String line = st.lyric.trim();
        int maxW = Math.min(340, screenW - 40);
        String trimmed = font.plainSubstrByWidth(line, maxW - 16);
        int textW = font.width(trimmed);
        int boxW = textW + 18;
        int boxX = (screenW - boxW) / 2;
        int boxY = screenH - config.lyricsYOffset;

        int bg = TntronThemeColors.withAlpha(0x0D0F14, config.hudOpacity * 0.85f);
        int border = TntronThemeColors.withAlpha(accent, 0.55f);
        g.fill(boxX, boxY, boxX + boxW, boxY + 15, bg);
        g.fill(boxX, boxY, boxX + 2, boxY + 15, accent);
        g.fill(boxX, boxY, boxX + boxW, boxY + 1, border);
        g.text(font, trimmed, boxX + 9, boxY + 3, 0xFFF8FAFC, true);
    }

    private void renderNowPlayingCard(GuiGraphicsExtractor g, Font font, int screenW, int screenH, TntronStatus st, int accent) {
        float scale = config.hudScale;
        int cardW = 198;
        int cardH = config.showSpectrum ? 64 : 46;
        int margin = 8;

        int scaledW = Math.round(cardW * scale);
        int scaledH = Math.round(cardH * scale);

        int x;
        int y;
        String pos = config.hudPosition == null ? "top_left" : config.hudPosition;
        switch (pos) {
            case "top_right" -> { x = screenW - scaledW - margin; y = margin; }
            case "bottom_left" -> { x = margin; y = screenH - scaledH - margin; }
            case "bottom_right" -> { x = screenW - scaledW - margin; y = screenH - scaledH - margin; }
            default -> { x = margin; y = margin; }
        }

        g.pose().pushMatrix();
        g.pose().translate(x, y);
        g.pose().scale(scale, scale);

        int bg = TntronThemeColors.withAlpha(0x0B0E14, config.hudOpacity);
        int bgHeader = TntronThemeColors.withAlpha(0x141923, config.hudOpacity);
        int border = TntronThemeColors.withAlpha(accent, 0.45f + Math.min(0.45f, smoothBeat * 0.45f));

        // Card background + accent outline
        g.fill(0, 0, cardW, cardH, bg);
        g.fill(0, 0, cardW, 2, border);
        g.fill(0, cardH - 1, cardW, cardH, TntronThemeColors.withAlpha(0xFFFFFF, 0.12f));
        g.fill(0, 0, 1, cardH, TntronThemeColors.withAlpha(0xFFFFFF, 0.12f));
        g.fill(cardW - 1, 0, cardW, cardH, TntronThemeColors.withAlpha(0xFFFFFF, 0.12f));

        int textLeft = 7;
        if (config.showCover) {
            int artSize = 34;
            int artX = 6;
            int artY = 6;
            g.fill(artX - 1, artY - 1, artX + artSize + 1, artY + artSize + 1, bgHeader);
            if (coverArt.hasCover()) {
                g.blit(RenderPipelines.GUI_TEXTURED, CoverArtManager.COVER_TEX_ID, artX, artY, 0f, 0f, artSize, artSize, artSize, artSize);
            } else {
                // Stylized vinyl disc fallback
                g.fill(artX, artY, artX + artSize, artY + artSize, 0xFF18181B);
                g.fill(artX + 5, artY + 5, artX + artSize - 5, artY + artSize - 5, 0xFF27272A);
                g.fill(artX + 12, artY + 12, artX + artSize - 12, artY + artSize - 12, accent);
                g.fill(artX + 15, artY + 15, artX + artSize - 15, artY + artSize - 15, 0xFF09090B);
            }
            textLeft = artX + artSize + 7;
        }

        int maxTextW = cardW - textLeft - 7;
        String title = st.title != null && !st.title.isEmpty() ? st.title : "No Track";
        String artist = st.artist != null && !st.artist.isEmpty() ? st.artist : "tntron";

        String stateIcon = st.isPlaying() ? "▶ " : (st.isPaused() ? "⏸ " : "■ ");
        if (st.is_favorite) stateIcon = "♥ " + stateIcon;
        String titleDraw = font.plainSubstrByWidth(stateIcon + title, maxTextW);
        String artistDraw = font.plainSubstrByWidth(artist, maxTextW);

        g.text(font, titleDraw, textLeft, 7, 0xFFF8FAFC, true);
        g.text(font, artistDraw, textLeft, 18, 0xFF94A3B8, false);

        if (config.showMetaBadges) {
            StringBuilder meta = new StringBuilder();
            if (st.bpm != null && st.bpm > 0) meta.append(Math.round(st.bpm)).append(" BPM");
            if (Math.abs(st.speed - 1.0) > 0.01) {
                if (!meta.isEmpty()) meta.append(" · ");
                meta.append(String.format("%.2fx", st.speed));
            }
            if (st.dsp_preset != null && !"Flat".equalsIgnoreCase(st.dsp_preset)) {
                if (!meta.isEmpty()) meta.append(" · ");
                meta.append(st.dsp_preset);
            }
            if (st.muted) {
                if (!meta.isEmpty()) meta.append(" · ");
                meta.append("MUTE");
            }
            String timeStr = TntronThemeColors.formatTime(interpolatedPos) + "/" + TntronThemeColors.formatTime(st.duration);
            String leftMeta = font.plainSubstrByWidth(meta.toString(), Math.max(20, maxTextW - font.width(timeStr) - 4));
            g.text(font, leftMeta, textLeft, 29, accent, false);
            g.text(font, timeStr, cardW - 6 - font.width(timeStr), 29, 0xFFCBD5E1, false);
        }

        if (config.showProgress) {
            int barX = 6;
            int barW = cardW - 12;
            int barY = 43;
            g.fill(barX, barY, barX + barW, barY + 2, 0x55FFFFFF);
            double frac = st.duration > 0 ? Math.max(0.0, Math.min(1.0, interpolatedPos / st.duration)) : 0.0;
            int fillW = Math.max(0, (int) Math.round(barW * frac));
            if (fillW > 0) {
                g.fill(barX, barY, barX + fillW, barY + 2, accent);
            }
        }

        if (config.showSpectrum) {
            int specX = 6;
            int specW = cardW - 12;
            int specBottom = cardH - 4;
            int maxBarH = 14;
            int count = 16;
            int gap = 2;
            int bw = (specW - (count - 1) * gap) / count;
            for (int i = 0; i < count; i++) {
                float v = smoothBands[i];
                int bh = Math.max(1, Math.round(v * maxBarH));
                int bx = specX + i * (bw + gap);
                int col = TntronThemeColors.blendRgb(accent, 0xFFFFFFFF, v * 0.45f);
                g.fill(bx, specBottom - bh, bx + bw, specBottom, col);
            }
        }

        g.pose().popMatrix();
    }
}
