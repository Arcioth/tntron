package com.arcioth.tntron.android;

import android.graphics.Color;

public final class ThemeColors {
    private ThemeColors() {}

    public static int resolveAccent(String slug) {
        if (slug == null) return 0xFF1DB954;
        switch (slug.toLowerCase()) {
            case "dark": return 0xFFE4E4E7;
            case "light": return 0xFF64748B;
            case "blue": return 0xFF3B82F6;
            case "purple": return 0xFFA855F7;
            case "red": return 0xFFEF4444;
            case "gold": return 0xFFEAB308;
            case "cyberpunk": return 0xFFFF007F;
            case "terminal": return 0xFF00FF00;
            case "pharoh": return 0xFFD4AF37;
            case "turk": return 0xFFE30A17;
            case "clouds": return 0xFF38BDF8;
            default: return 0xFF1DB954;
        }
    }

    public static int parseHexOr(String hex, int fallback) {
        if (hex == null || hex.trim().isEmpty()) return fallback;
        try {
            String s = hex.trim();
            if (!s.startsWith("#")) s = "#" + s;
            return Color.parseColor(s);
        } catch (Exception ignored) {
            return fallback;
        }
    }

    public static String formatTime(double seconds) {
        if (seconds < 0 || Double.isNaN(seconds) || Double.isInfinite(seconds)) seconds = 0;
        long total = (long) Math.floor(seconds);
        long mins = total / 60;
        long secs = total % 60;
        return String.format("%d:%02d", mins, secs);
    }
}
