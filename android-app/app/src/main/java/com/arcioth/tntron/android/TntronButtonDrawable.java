package com.arcioth.tntron.android;

import android.graphics.Canvas;
import android.graphics.Color;
import android.graphics.ColorFilter;
import android.graphics.LinearGradient;
import android.graphics.Paint;
import android.graphics.Path;
import android.graphics.PixelFormat;
import android.graphics.Rect;
import android.graphics.RectF;
import android.graphics.Shader;
import android.graphics.drawable.Drawable;

import androidx.annotation.NonNull;
import androidx.annotation.Nullable;

/**
 * Renders buttons matching tntron's host ButtonPackConfig:
 * selective per-corner rounding, cyberpunk corner cropping, gradients, outline, and glow.
 */
public final class TntronButtonDrawable extends Drawable {
    private final HostUiStyle style;
    private final boolean primary;
    private final float density;

    private final Paint fillPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint strokePaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Path path = new Path();
    private final RectF boundsF = new RectF();

    public TntronButtonDrawable(HostUiStyle style, boolean primary, float density) {
        this.style = style;
        this.primary = primary;
        this.density = density;
        strokePaint.setStyle(Paint.Style.STROKE);
    }

    private float dp(float v) {
        return v * density;
    }

    @Override
    protected void onBoundsChange(@NonNull Rect bounds) {
        super.onBoundsChange(bounds);
        boundsF.set(bounds);
        rebuildPath();
        rebuildPaints();
    }

    private void rebuildPath() {
        path.reset();
        float l = boundsF.left + dp(1f);
        float t = boundsF.top + dp(1f);
        float r = boundsF.right - dp(1f);
        float b = boundsF.bottom - dp(1f);
        float w = Math.max(1f, r - l);
        float h = Math.max(1f, b - t);

        boolean usePack = style.buttonEnabled;
        boolean hasCrop = usePack && (style.cropTl || style.cropTr || style.cropBr || style.cropBl
                || "cyberpunk".equalsIgnoreCase(style.buttonPreset));

        if (hasCrop) {
            float cut = Math.min(Math.min(w, h) * 0.38f, dp(Math.max(4f, style.cropSize)));
            float cTl = (style.cropTl || "cyberpunk".equalsIgnoreCase(style.buttonPreset)) ? cut : 0f;
            float cTr = style.cropTr ? cut : 0f;
            float cBr = (style.cropBr || "cyberpunk".equalsIgnoreCase(style.buttonPreset)) ? cut : 0f;
            float cBl = style.cropBl ? cut : 0f;

            path.moveTo(l + cTl, t);
            path.lineTo(r - cTr, t);
            if (cTr > 0) path.lineTo(r, t + cTr);
            path.lineTo(r, b - cBr);
            if (cBr > 0) path.lineTo(r - cBr, b);
            path.lineTo(l + cBl, b);
            if (cBl > 0) path.lineTo(l, b - cBl);
            path.lineTo(l, t + cTl);
            path.close();
        } else {
            float maxR = Math.min(w, h) * 0.5f;
            float rTl = usePack ? Math.min(maxR, dp(Math.max(0f, style.roundTl))) : Math.min(maxR, dp(10f));
            float rTr = usePack ? Math.min(maxR, dp(Math.max(0f, style.roundTr))) : Math.min(maxR, dp(10f));
            float rBr = usePack ? Math.min(maxR, dp(Math.max(0f, style.roundBr))) : Math.min(maxR, dp(10f));
            float rBl = usePack ? Math.min(maxR, dp(Math.max(0f, style.roundBl))) : Math.min(maxR, dp(10f));
            float[] radii = {rTl, rTl, rTr, rTr, rBr, rBr, rBl, rBl};
            path.addRoundRect(new RectF(l, t, r, b), radii, Path.Direction.CW);
        }
    }

    private void rebuildPaints() {
        int baseFill = primary ? style.accent : style.cardHover;
        if (style.buttonEnabled && style.customTint != 0 && !primary) {
            baseFill = blend(baseFill, style.customTint, 0.35f);
        }

        if (style.buttonEnabled && (style.gradientEnabled
                || "liquid_glass".equalsIgnoreCase(style.buttonPreset)
                || "windows_xp".equalsIgnoreCase(style.buttonPreset))) {
            int topCol = style.gradientTop != 0 ? style.gradientTop : lighten(baseFill, 0.18f);
            int botCol = style.gradientBottom != 0 ? style.gradientBottom : darken(baseFill, 0.18f);
            fillPaint.setShader(new LinearGradient(
                    boundsF.left, boundsF.top, boundsF.left, boundsF.bottom,
                    topCol, botCol, Shader.TileMode.CLAMP));
        } else {
            fillPaint.setShader(null);
            fillPaint.setColor(baseFill);
        }

        int alpha = Math.max(80, Math.min(255, Math.round(style.fillOpacity * 255f)));
        fillPaint.setAlpha(alpha);

        if (style.buttonEnabled && (style.outlineEnabled || style.outerGlow > 0.05f)) {
            int strokeCol = style.outlineColor != 0 ? style.outlineColor : style.accent;
            strokePaint.setColor(strokeCol);
            strokePaint.setAlpha(primary ? 230 : 185);
            strokePaint.setStrokeWidth(dp(Math.max(1f, style.outlineWidth)));
        } else {
            strokePaint.setColor(style.isLight ? 0x26000000 : 0x26FFFFFF);
            strokePaint.setStrokeWidth(dp(1f));
        }
    }

    @Override
    public void draw(@NonNull Canvas canvas) {
        canvas.drawPath(path, fillPaint);
        canvas.drawPath(path, strokePaint);
    }

    @Override public void setAlpha(int alpha) { fillPaint.setAlpha(alpha); }
    @Override public void setColorFilter(@Nullable ColorFilter cf) { fillPaint.setColorFilter(cf); }
    @Override public int getOpacity() { return PixelFormat.TRANSLUCENT; }

    private static int blend(int c1, int c2, float t) {
        int r = Math.round(Color.red(c1) + (Color.red(c2) - Color.red(c1)) * t);
        int g = Math.round(Color.green(c1) + (Color.green(c2) - Color.green(c1)) * t);
        int b = Math.round(Color.blue(c1) + (Color.blue(c2) - Color.blue(c1)) * t);
        return Color.rgb(r, g, b);
    }

    private static int lighten(int c, float t) { return blend(c, Color.WHITE, t); }
    private static int darken(int c, float t) { return blend(c, Color.BLACK, t); }
}
