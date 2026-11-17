package com.arcioth.tntron.render;

import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronModels.ThemeItem;

import java.util.List;

public final class TntronThemeColors {
    private TntronThemeColors() {}

    public static int resolveAccent(TntronModConfig cfg, String activeSlug, List<ThemeItem> themes) {
        if (cfg != null && !cfg.syncAccentWithTheme) {
            return parseHexRgb(cfg.customAccentHex, 0xFF1DB954);
        }
        if (activeSlug != null && themes != null) {
            for (ThemeItem t : themes) {
                if (activeSlug.equalsIgnoreCase(t.slug) && t.accent != null && !t.accent.isBlank()) {
                    return parseHexRgb(t.accent, 0xFF1DB954);
                }
            }
        }
        if (activeSlug == null) return 0xFF1DB954;
        return switch (activeSlug.toLowerCase()) {
            case "dark" -> 0xFFE4E4E7;
            case "light" -> 0xFF3F3F46;
            case "blue" -> 0xFF3B82F6;
            case "purple" -> 0xFFA855F7;
            case "red" -> 0xFFEF4444;
            case "gold" -> 0xFFEAB308;
            case "cyberpunk" -> 0xFFFF007F;
            case "terminal" -> 0xFF00FF00;
            case "pharoh" -> 0xFFD4AF37;
            case "turk" -> 0xFFE30A17;
            case "clouds" -> 0xFF38BDF8;
            default -> 0xFF1DB954;
        };
    }

    public static int parseHexRgb(String hex, int fallback) {
        if (hex == null) return fallback;
        String s = hex.trim();
        if (s.startsWith("#")) s = s.substring(1);
        if (s.length() == 6) {
            try {
                return 0xFF000000 | Integer.parseInt(s, 16);
            } catch (NumberFormatException ignored) {
            }
        }
        return fallback;
    }

    public static int withAlpha(int argb, float alpha01) {
        int a = Math.max(0, Math.min(255, Math.round(alpha01 * 255.0f)));
        return (a << 24) | (argb & 0x00FFFFFF);
    }

    public static int blendRgb(int c1, int c2, float t) {
        t = Math.max(0f, Math.min(1f, t));
        int r1 = (c1 >> 16) & 0xFF, g1 = (c1 >> 8) & 0xFF, b1 = c1 & 0xFF;
        int r2 = (c2 >> 16) & 0xFF, g2 = (c2 >> 8) & 0xFF, b2 = c2 & 0xFF;
        int r = Math.round(r1 + (r2 - r1) * t);
        int g = Math.round(g1 + (g2 - g1) * t);
        int b = Math.round(b1 + (b2 - b1) * t);
        return 0xFF000000 | (r << 16) | (g << 8) | b;
    }

    public static String formatTime(double seconds) {
        if (seconds < 0 || Double.isNaN(seconds) || Double.isInfinite(seconds)) seconds = 0;
        long total = (long) Math.floor(seconds);
        long mins = total / 60;
        long secs = total % 60;
        return String.format("%d:%02d", mins, secs);
    }
}
