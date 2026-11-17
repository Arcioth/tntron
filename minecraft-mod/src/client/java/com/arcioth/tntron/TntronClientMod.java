package com.arcioth.tntron;

import com.arcioth.tntron.command.TntronCommands;
import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronClient;
import com.arcioth.tntron.ipc.TntronModels.*;
import com.arcioth.tntron.render.CoverArtManager;
import com.arcioth.tntron.render.TntronHudOverlay;
import com.arcioth.tntron.ui.TntronDashboardScreen;
import net.fabricmc.api.ClientModInitializer;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientLifecycleEvents;
import net.fabricmc.fabric.api.client.event.lifecycle.v1.ClientTickEvents;
import net.fabricmc.fabric.api.client.keymapping.v1.KeyMappingHelper;
import net.fabricmc.fabric.api.client.rendering.v1.hud.HudElementRegistry;
import net.minecraft.ChatFormatting;
import net.minecraft.client.KeyMapping;
import net.minecraft.client.Minecraft;
import net.minecraft.client.gui.components.toasts.SystemToast;
import net.minecraft.core.particles.ParticleTypes;
import net.minecraft.network.chat.Component;
import net.minecraft.resources.Identifier;
import net.minecraft.sounds.SoundSource;
import net.minecraft.world.entity.Entity;
import net.minecraft.world.entity.item.PrimedTnt;
import org.lwjgl.glfw.GLFW;

public final class TntronClientMod implements ClientModInitializer {
    private static final SystemToast.SystemToastId TOAST_ID = new SystemToast.SystemToastId(4500L);
    private static final KeyMapping.Category KEY_CATEGORY =
            KeyMapping.Category.register(Identifier.fromNamespaceAndPath("tntron", "controls"));

    private TntronModConfig config;
    private TntronClient client;
    private CoverArtManager coverArt;
    private TntronHudOverlay hudOverlay;

    private KeyMapping keyOpenPanel;
    private KeyMapping keyPlayPause;
    private KeyMapping keyPrevTrack;
    private KeyMapping keyNextTrack;
    private KeyMapping keyToggleHud;
    private KeyMapping keyToggleLyrics;
    private KeyMapping keyToggleFavorite;
    private KeyMapping keyVolUp;
    private KeyMapping keyVolDown;
    private KeyMapping keyMute;

    private String lastTrackKey = "";
    private long lastNoticeSeq = 0;
    private int tickCounter = 0;
    private boolean wasInWorld = false;
    private boolean wasTntNearby = false;
    private boolean wasInCombat = false;
    private boolean mcMusicMutedByMod = false;
    private int savedFov = -1;

    @Override
    public void onInitializeClient() {
        config = TntronModConfig.load();
        coverArt = new CoverArtManager();
        client = new TntronClient(config, this::onStatusUpdate);
        hudOverlay = new TntronHudOverlay(client, config, coverArt);

        registerKeyMappings();
        HudElementRegistry.addLast(Identifier.fromNamespaceAndPath("tntron", "hud"), hudOverlay);
        TntronCommands.register(client, config, coverArt);

        ClientTickEvents.END_CLIENT_TICK.register(this::onClientTick);
        ClientLifecycleEvents.CLIENT_STOPPING.register(mc -> {
            restoreMcMusicVolume(mc);
            restoreFov(mc);
            config.save();
            client.stop();
            coverArt.shutdown();
        });

        client.start();
    }

    private void registerKeyMappings() {
        keyOpenPanel = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Open tntron Control Center", GLFW.GLFW_KEY_F8, KEY_CATEGORY));
        keyPrevTrack = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Previous Track", GLFW.GLFW_KEY_F6, KEY_CATEGORY));
        keyPlayPause = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Play / Pause", GLFW.GLFW_KEY_F7, KEY_CATEGORY));
        keyNextTrack = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Next Track", GLFW.GLFW_KEY_F9, KEY_CATEGORY));
        keyToggleHud = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Toggle HUD Overlay", GLFW.GLFW_KEY_F10, KEY_CATEGORY));
        keyToggleLyrics = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Toggle Lyrics Overlay", GLFW.GLFW_KEY_UNKNOWN, KEY_CATEGORY));
        keyToggleFavorite = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Toggle Track Favorite", GLFW.GLFW_KEY_UNKNOWN, KEY_CATEGORY));
        keyVolUp = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Volume Up (+5%)", GLFW.GLFW_KEY_UNKNOWN, KEY_CATEGORY));
        keyVolDown = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Volume Down (-5%)", GLFW.GLFW_KEY_UNKNOWN, KEY_CATEGORY));
        keyMute = KeyMappingHelper.registerKeyMapping(new KeyMapping(
                "Mute / Unmute", GLFW.GLFW_KEY_UNKNOWN, KEY_CATEGORY));
    }

    private void onStatusUpdate(TntronStatus st) {
        String key = (st.track_path != null ? st.track_path : "") + "|" + (st.title != null ? st.title : "");
        if (!key.equals(lastTrackKey)) {
            lastTrackKey = key;
            if (st.hasTrack()) {
                client.fetchCoverBase64(b64 -> coverArt.loadFromBase64(key, b64));
                Minecraft.getInstance().execute(() -> notifyTrackChange(st));
            } else {
                Minecraft.getInstance().execute(() -> coverArt.clearOnRenderThread());
            }
        }

        if (st.mc_notice != null && st.mc_notice.seq > lastNoticeSeq) {
            lastNoticeSeq = st.mc_notice.seq;
            McNotice notice = st.mc_notice;
            Minecraft.getInstance().execute(() -> showNotice(notice.kind, notice.title, notice.body));
        }
    }

    private void notifyTrackChange(TntronStatus st) {
        if (config.notifyMode == null || "off".equalsIgnoreCase(config.notifyMode)) return;
        String artist = (st.artist != null && !st.artist.isBlank() && !"Unknown Artist".equals(st.artist))
                ? st.artist : "tntron";
        showNotice(config.notifyMode, st.title, artist);
    }

    private void showNotice(String kind, String title, String body) {
        Minecraft mc = Minecraft.getInstance();
        String safeTitle = title == null ? "tntron" : title;
        String safeBody = body == null ? "" : body;

        if ("chat".equalsIgnoreCase(kind)) {
            if (mc.player != null) {
                mc.player.sendSystemMessage(
                        Component.literal("[tntron] ").withStyle(ChatFormatting.GREEN)
                                .append(Component.literal(safeTitle).withStyle(ChatFormatting.WHITE))
                                .append(safeBody.isEmpty() ? Component.empty()
                                        : Component.literal(" — " + safeBody).withStyle(ChatFormatting.GRAY))
                );
            }
        } else if ("actionbar".equalsIgnoreCase(kind)) {
            String msg = safeBody.isEmpty() ? ("♪ " + safeTitle) : ("♪ " + safeTitle + " — " + safeBody);
            mc.gui.hud.setOverlayMessage(Component.literal(msg).withStyle(ChatFormatting.AQUA), false);
        } else {
            SystemToast.addOrUpdate(mc.gui.toastManager(), TOAST_ID,
                    Component.literal(safeTitle), Component.literal(safeBody));
        }
    }

    private void onClientTick(Minecraft mc) {
        tickCounter++;

        while (keyOpenPanel.consumeClick()) {
            if (mc.gui.screen() instanceof TntronDashboardScreen) {
                mc.gui.setScreen(null);
            } else {
                mc.gui.setScreen(new TntronDashboardScreen(client, config, coverArt));
            }
        }
        while (keyPlayPause.consumeClick()) client.toggle();
        while (keyPrevTrack.consumeClick()) client.prev();
        while (keyNextTrack.consumeClick()) client.next();
        while (keyToggleHud.consumeClick()) {
            config.hudEnabled = !config.hudEnabled;
            config.save();
            mc.gui.hud.setOverlayMessage(
                    Component.literal("tntron HUD: " + (config.hudEnabled ? "ON" : "OFF")), false);
        }
        while (keyToggleLyrics.consumeClick()) {
            config.lyricsOverlayEnabled = !config.lyricsOverlayEnabled;
            config.save();
            mc.gui.hud.setOverlayMessage(
                    Component.literal("tntron Lyrics: " + (config.lyricsOverlayEnabled ? "ON" : "OFF")), false);
        }
        while (keyToggleFavorite.consumeClick()) client.toggleFavorite(null);
        while (keyVolUp.consumeClick()) client.volumeDelta(5);
        while (keyVolDown.consumeClick()) client.volumeDelta(-5);
        while (keyMute.consumeClick()) client.setMute(null);

        TntronStatus st = client.getStatus();
        boolean playing = client.isConnected() && st.isPlaying();

        // Auto-mute vanilla Minecraft music while tntron is playing
        updateMcMusicMute(mc, playing);

        // Beat-reactive world particles & subtle FOV pulse
        updateReactiveEffects(mc, playing, st);

        // Send live Minecraft telemetry every 10 ticks (~500ms) + instant events
        if (config.sendTelemetry && client.isConnected()) {
            collectAndSendTelemetry(mc);
        }
    }

    private void updateMcMusicMute(Minecraft mc, boolean tntronPlaying) {
        var musicOpt = mc.options.getSoundSourceOptionInstance(SoundSource.MUSIC);
        if (config.autoMuteMcMusic && tntronPlaying) {
            double cur = musicOpt.get();
            if (!mcMusicMutedByMod && cur > 0.001) {
                config.savedMcMusicVolume = cur;
                config.save();
                musicOpt.set(0.0);
                mcMusicMutedByMod = true;
            }
        } else {
            restoreMcMusicVolume(mc);
        }
    }

    private void restoreMcMusicVolume(Minecraft mc) {
        if (mcMusicMutedByMod) {
            var musicOpt = mc.options.getSoundSourceOptionInstance(SoundSource.MUSIC);
            double restore = config.savedMcMusicVolume >= 0.0 ? config.savedMcMusicVolume : 1.0;
            musicOpt.set(restore);
            mcMusicMutedByMod = false;
        }
    }

    private void updateReactiveEffects(Minecraft mc, boolean playing, TntronStatus st) {
        if (config.reactiveEnabled && config.beatParticles && playing && mc.level != null && mc.player != null) {
            float beat = st.beat;
            if (beat > 0.35f && tickCounter % 3 == 0) {
                var rng = mc.level.getRandom();
                double px = mc.player.getX() + (rng.nextDouble() - 0.5) * 1.8;
                double py = mc.player.getY() + 0.6 + rng.nextDouble() * 1.4;
                double pz = mc.player.getZ() + (rng.nextDouble() - 0.5) * 1.8;
                double colorNote = (tickCounter % 24) / 24.0;
                mc.level.addParticle(ParticleTypes.NOTE, px, py, pz, colorNote, 0.0, 0.0);
            }
        }

        if (config.reactiveEnabled && config.beatFovPulse && playing) {
            var fovOpt = mc.options.fov();
            if (savedFov < 0) savedFov = fovOpt.get();
            int pulse = Math.round(hudOverlay.getSmoothBeat() * 3.0f * config.beatIntensity);
            fovOpt.set(Math.max(30, Math.min(110, savedFov + pulse)));
        } else {
            restoreFov(mc);
        }
    }

    private void restoreFov(Minecraft mc) {
        if (savedFov >= 30) {
            mc.options.fov().set(savedFov);
            savedFov = -1;
        }
    }

    private void collectAndSendTelemetry(Minecraft mc) {
        boolean inWorld = mc.level != null && mc.player != null;
        if (inWorld && !wasInWorld) {
            client.sendMcEvent("join_world", mc.level.dimension().identifier().toString());
        } else if (!inWorld && wasInWorld) {
            client.sendMcEvent("leave_world", null);
        }
        wasInWorld = inWorld;

        boolean tntNearby = false;
        boolean inCombat = false;
        if (inWorld) {
            inCombat = mc.player.hurtTime > 0 || mc.player.getLastHurtByMob() != null;
            for (Entity e : mc.level.entitiesForRendering()) {
                if (e instanceof PrimedTnt && e.distanceToSqr(mc.player) <= 144.0) {
                    tntNearby = true;
                    break;
                }
            }
            if (tntNearby && !wasTntNearby) {
                client.sendMcEvent("tnt_primed", null);
            }
            if (inCombat && !wasInCombat) {
                client.sendMcEvent("combat_start", null);
            }
        }
        wasTntNearby = tntNearby;
        wasInCombat = inCombat;

        if (tickCounter % 10 != 0) return;

        McTelemetry tel = new McTelemetry();
        tel.player_name = mc.getUser().getName();
        tel.paused = mc.gui.isPausing();
        tel.in_world = inWorld;
        tel.in_combat = inCombat;
        tel.tnt_nearby = tntNearby;

        if (inWorld) {
            tel.dimension = mc.level.dimension().identifier().toString();
            var biomeHolder = mc.level.getBiome(mc.player.blockPosition());
            tel.biome = biomeHolder.unwrapKey().map(k -> k.identifier().toString()).orElse("unknown");
            tel.health = mc.player.getHealth();
            tel.max_health = mc.player.getMaxHealth();
            tel.boss_active = mc.gui.hud.getBossOverlay().shouldPlayMusic()
                    || mc.gui.hud.getBossOverlay().shouldDarkenScreen();
            tel.underwater = mc.player.isUnderWater();
            tel.weather = mc.level.isThundering() ? "thunder" : (mc.level.isRaining() ? "rain" : "clear");
            tel.speed_bps = (float) (mc.player.getDeltaMovement().horizontalDistance() * 20.0);
            tel.game_time = mc.level.getGameTime();
        }
        client.sendTelemetry(tel);
    }
}
