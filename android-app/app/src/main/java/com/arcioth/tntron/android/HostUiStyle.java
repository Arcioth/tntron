package com.arcioth.tntron.android;

import org.json.JSONObject;

public final class HostUiStyle {
    public String themeSlug = "openify";
    public String themeLabel = "Openify";
    public boolean isLight = false;

    public int base = 0xFF0A0D14;
    public int sidebar = 0xFF101521;
    public int surface = 0xFF111622;
    public int card = 0xFF171D2D;
    public int cardHover = 0xFF232B3E;
    public int text = 0xFFF8FAFC;
    public int muted = 0xFF94A3B8;
    public int accent = 0xFF1DB954;
    public int accentFg = 0xFF06100A;
    public int logoColor = 0xFF1DB954;

    // Button Pack
    public boolean buttonEnabled = false;
    public String buttonPreset = "openify";
    public float roundTl = 10f;
    public float roundTr = 10f;
    public float roundBr = 10f;
    public float roundBl = 10f;
    public boolean cropTl = false;
    public boolean cropTr = false;
    public boolean cropBr = false;
    public boolean cropBl = false;
    public float cropSize = 8f;
    public boolean outlineEnabled = false;
    public float outlineWidth = 1f;
    public int outlineColor = 0;
    public float outerGlow = 0f;
    public boolean gradientEnabled = false;
    public int gradientTop = 0;
    public int gradientBottom = 0;
    public int customTint = 0;
    public float fillOpacity = 1f;

    public boolean reactiveEnabled = false;
    public boolean hueLoop = true;
    public boolean bassGlow = true;
    public String eqStyle = "bars";
    public String fingerprint = "default";

    public static HostUiStyle fromJson(JSONObject obj) {
        HostUiStyle s = new HostUiStyle();
        if (obj == null) return s;

        s.themeSlug = obj.optString("theme_slug", s.themeSlug);
        s.themeLabel = obj.optString("theme_label", s.themeLabel);
        s.isLight = obj.optBoolean("is_light", false);

        s.base = ThemeColors.parseHexOr(obj.optString("base", ""), s.base);
        s.sidebar = ThemeColors.parseHexOr(obj.optString("sidebar", ""), s.sidebar);
        s.surface = ThemeColors.parseHexOr(obj.optString("surface", ""), s.surface);
        s.card = ThemeColors.parseHexOr(obj.optString("card", ""), s.card);
        s.cardHover = ThemeColors.parseHexOr(obj.optString("card_hover", ""), s.cardHover);
        s.text = ThemeColors.parseHexOr(obj.optString("text", ""), s.text);
        s.muted = ThemeColors.parseHexOr(obj.optString("muted", ""), s.muted);
        s.accent = ThemeColors.parseHexOr(obj.optString("accent", ""), s.accent);
        s.accentFg = ThemeColors.parseHexOr(obj.optString("accent_fg", ""), s.accentFg);
        s.logoColor = ThemeColors.parseHexOr(obj.optString("logo_color", ""), s.accent);

        JSONObject bp = obj.optJSONObject("button_pack");
        if (bp != null) {
            s.buttonEnabled = bp.optBoolean("enabled", false);
            s.buttonPreset = bp.optString("preset", "openify");
            s.roundTl = (float) bp.optDouble("round_tl", 10.0);
            s.roundTr = (float) bp.optDouble("round_tr", 10.0);
            s.roundBr = (float) bp.optDouble("round_br", 10.0);
            s.roundBl = (float) bp.optDouble("round_bl", 10.0);
            s.cropTl = bp.optBoolean("crop_tl", false);
            s.cropTr = bp.optBoolean("crop_tr", false);
            s.cropBr = bp.optBoolean("crop_br", false);
            s.cropBl = bp.optBoolean("crop_bl", false);
            s.cropSize = (float) bp.optDouble("crop_size", 8.0);
            s.outlineEnabled = bp.optBoolean("outline_enabled", false);
            s.outlineWidth = (float) bp.optDouble("outline_width", 1.2);
            if (!bp.isNull("outline_hex")) {
                s.outlineColor = ThemeColors.parseHexOr(bp.optString("outline_hex", ""), 0);
            }
            s.outerGlow = (float) bp.optDouble("outer_glow", 0.0);
            s.gradientEnabled = bp.optBoolean("gradient_enabled", false);
            if (!bp.isNull("gradient_top_hex")) {
                s.gradientTop = ThemeColors.parseHexOr(bp.optString("gradient_top_hex", ""), 0);
            }
            if (!bp.isNull("gradient_bottom_hex")) {
                s.gradientBottom = ThemeColors.parseHexOr(bp.optString("gradient_bottom_hex", ""), 0);
            }
            if (!bp.isNull("custom_tint_hex")) {
                s.customTint = ThemeColors.parseHexOr(bp.optString("custom_tint_hex", ""), 0);
            }
            s.fillOpacity = (float) bp.optDouble("fill_opacity", 1.0);
        }

        s.reactiveEnabled = obj.optBoolean("reactive_enabled", false);
        s.hueLoop = obj.optBoolean("hue_loop", true);
        s.bassGlow = obj.optBoolean("bass_glow", true);
        s.eqStyle = obj.optString("eq_style", "bars");
        s.fingerprint = obj.toString();
        return s;
    }
}
