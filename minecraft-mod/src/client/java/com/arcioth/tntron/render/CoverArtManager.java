package com.arcioth.tntron.render;

import com.mojang.blaze3d.platform.NativeImage;
import net.minecraft.client.Minecraft;
import net.minecraft.client.renderer.texture.DynamicTexture;
import net.minecraft.resources.Identifier;

import javax.imageio.ImageIO;
import java.awt.*;
import java.awt.image.BufferedImage;
import java.io.ByteArrayInputStream;
import java.io.ByteArrayOutputStream;
import java.util.Base64;
import java.util.concurrent.ExecutorService;
import java.util.concurrent.Executors;

public final class CoverArtManager {
    public static final Identifier COVER_TEX_ID = Identifier.fromNamespaceAndPath("tntron", "dynamic_cover");

    private final ExecutorService decodeExecutor = Executors.newSingleThreadExecutor(r -> {
        Thread t = new Thread(r, "tntron-cover-decode");
        t.setDaemon(true);
        return t;
    });

    private volatile boolean hasCover = false;
    private volatile String currentTrackKey = "";
    private DynamicTexture activeTexture;

    public boolean hasCover() {
        return hasCover;
    }

    public String getCurrentTrackKey() {
        return currentTrackKey;
    }

    public void clearOnRenderThread() {
        hasCover = false;
        currentTrackKey = "";
        if (activeTexture != null) {
            Minecraft.getInstance().getTextureManager().release(COVER_TEX_ID);
            activeTexture.close();
            activeTexture = null;
        }
    }

    public void loadFromBase64(String trackKey, String base64) {
        currentTrackKey = trackKey == null ? "" : trackKey;
        if (base64 == null || base64.isBlank()) {
            Minecraft.getInstance().execute(this::clearKeepingKey);
            return;
        }
        decodeExecutor.submit(() -> {
            try {
                byte[] raw = Base64.getDecoder().decode(base64);
                BufferedImage img = ImageIO.read(new ByteArrayInputStream(raw));
                if (img == null) {
                    Minecraft.getInstance().execute(this::clearKeepingKey);
                    return;
                }
                int target = 128;
                BufferedImage scaled = new BufferedImage(target, target, BufferedImage.TYPE_INT_ARGB);
                Graphics2D g = scaled.createGraphics();
                g.setRenderingHint(RenderingHints.KEY_INTERPOLATION, RenderingHints.VALUE_INTERPOLATION_BILINEAR);
                g.drawImage(img, 0, 0, target, target, null);
                g.dispose();

                ByteArrayOutputStream pngOut = new ByteArrayOutputStream();
                ImageIO.write(scaled, "png", pngOut);
                byte[] pngBytes = pngOut.toByteArray();

                Minecraft.getInstance().execute(() -> {
                    if (!currentTrackKey.equals(trackKey == null ? "" : trackKey)) {
                        return;
                    }
                    try {
                        NativeImage ni = NativeImage.read(pngBytes);
                        if (activeTexture != null) {
                            Minecraft.getInstance().getTextureManager().release(COVER_TEX_ID);
                            activeTexture.close();
                        }
                        activeTexture = new DynamicTexture(() -> "tntron_cover", ni);
                        Minecraft.getInstance().getTextureManager().register(COVER_TEX_ID, activeTexture);
                        hasCover = true;
                    } catch (Throwable ignored) {
                        clearKeepingKey();
                    }
                });
            } catch (Throwable ignored) {
                Minecraft.getInstance().execute(this::clearKeepingKey);
            }
        });
    }

    private void clearKeepingKey() {
        hasCover = false;
        if (activeTexture != null) {
            Minecraft.getInstance().getTextureManager().release(COVER_TEX_ID);
            activeTexture.close();
            activeTexture = null;
        }
    }

    public void shutdown() {
        decodeExecutor.shutdownNow();
        Minecraft mc = Minecraft.getInstance();
        if (mc != null) {
            mc.execute(this::clearOnRenderThread);
        }
    }
}
