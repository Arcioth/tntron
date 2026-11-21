package com.arcioth.tntron.android;

import android.content.Context;
import android.graphics.Canvas;
import android.graphics.Paint;
import android.graphics.RectF;
import android.view.View;

public final class SpectrumView extends View {
    private final float[] targetBands = new float[16];
    private final float[] smoothBands = new float[16];
    private final Paint barPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final Paint bgPaint = new Paint(Paint.ANTI_ALIAS_FLAG);
    private final RectF rect = new RectF();
    private int accentColor = 0xFF1DB954;
    private boolean playing = false;

    public SpectrumView(Context context) {
        super(context);
        bgPaint.setColor(0xFF0E1320);
        barPaint.setColor(accentColor);
    }

    public void updateSpectrum(float[] bands, int accent, boolean isPlaying) {
        this.accentColor = accent;
        this.playing = isPlaying;
        for (int i = 0; i < 16; i++) {
            float v = (isPlaying && bands != null && i < bands.length) ? bands[i] : 0f;
            targetBands[i] = Math.max(0f, Math.min(1f, v));
        }
        invalidate();
    }

    @Override
    protected void onDraw(Canvas canvas) {
        super.onDraw(canvas);
        float w = getWidth();
        float h = getHeight();
        if (w <= 0 || h <= 0) return;

        rect.set(0, 0, w, h);
        canvas.drawRoundRect(rect, 20f, 20f, bgPaint);

        float padX = 16f;
        float padY = 12f;
        float usableW = w - padX * 2f;
        float usableH = h - padY * 2f;
        float gap = 8f;
        float barW = (usableW - gap * 15f) / 16f;
        boolean needsMore = false;

        for (int i = 0; i < 16; i++) {
            float diff = targetBands[i] - smoothBands[i];
            smoothBands[i] += diff * (diff > 0 ? 0.45f : 0.22f);
            if (Math.abs(diff) > 0.01f) needsMore = true;

            float bh = Math.max(6f, smoothBands[i] * usableH);
            float left = padX + i * (barW + gap);
            float top = h - padY - bh;
            rect.set(left, top, left + barW, h - padY);
            barPaint.setColor(accentColor);
            barPaint.setAlpha(playing ? (int) (140 + smoothBands[i] * 115) : 70);
            canvas.drawRoundRect(rect, barW * 0.4f, barW * 0.4f, barPaint);
        }

        if (needsMore) {
            postInvalidateOnAnimation();
        }
    }
}
