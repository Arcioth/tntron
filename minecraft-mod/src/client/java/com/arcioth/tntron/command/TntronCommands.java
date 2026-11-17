package com.arcioth.tntron.command;

import com.arcioth.tntron.config.TntronModConfig;
import com.arcioth.tntron.ipc.TntronClient;
import com.arcioth.tntron.ipc.TntronModels.*;
import com.arcioth.tntron.render.CoverArtManager;
import com.arcioth.tntron.render.TntronThemeColors;
import com.arcioth.tntron.ui.TntronDashboardScreen;
import com.mojang.brigadier.CommandDispatcher;
import com.mojang.brigadier.arguments.DoubleArgumentType;
import com.mojang.brigadier.arguments.IntegerArgumentType;
import com.mojang.brigadier.arguments.StringArgumentType;
import net.fabricmc.fabric.api.client.command.v2.ClientCommandRegistrationCallback;
import net.fabricmc.fabric.api.client.command.v2.ClientCommands;
import net.fabricmc.fabric.api.client.command.v2.FabricClientCommandSource;
import net.minecraft.ChatFormatting;
import net.minecraft.client.Minecraft;
import net.minecraft.commands.CommandBuildContext;
import net.minecraft.network.chat.ClickEvent;
import net.minecraft.network.chat.Component;
import net.minecraft.network.chat.MutableComponent;

public final class TntronCommands {
    private TntronCommands() {}

    public static void register(TntronClient client, TntronModConfig config, CoverArtManager coverArt) {
        ClientCommandRegistrationCallback.EVENT.register((dispatcher, registryAccess) ->
                registerTree(dispatcher, registryAccess, client, config, coverArt));
    }

    private static void registerTree(
            CommandDispatcher<FabricClientCommandSource> dispatcher,
            CommandBuildContext registryAccess,
            TntronClient client,
            TntronModConfig config,
            CoverArtManager coverArt
    ) {
        dispatcher.register(ClientCommands.literal("tntron")
                .executes(ctx -> {
                    Minecraft mc = ctx.getSource().getClient();
                    mc.execute(() -> mc.gui.setScreen(new TntronDashboardScreen(client, config, coverArt)));
                    return 1;
                })
                .then(ClientCommands.literal("play").executes(ctx -> {
                    client.play();
                    ctx.getSource().sendFeedback(msg("Resumed playback"));
                    return 1;
                }))
                .then(ClientCommands.literal("pause").executes(ctx -> {
                    client.pause();
                    ctx.getSource().sendFeedback(msg("Paused playback"));
                    return 1;
                }))
                .then(ClientCommands.literal("toggle").executes(ctx -> {
                    client.toggle();
                    ctx.getSource().sendFeedback(msg("Toggled play/pause"));
                    return 1;
                }))
                .then(ClientCommands.literal("stop").executes(ctx -> {
                    client.stopPlayback();
                    ctx.getSource().sendFeedback(msg("Stopped playback"));
                    return 1;
                }))
                .then(ClientCommands.literal("next").executes(ctx -> {
                    client.next();
                    ctx.getSource().sendFeedback(msg("Skipped to next track"));
                    return 1;
                }))
                .then(ClientCommands.literal("prev").executes(ctx -> {
                    client.prev();
                    ctx.getSource().sendFeedback(msg("Returned to previous track"));
                    return 1;
                }))
                .then(ClientCommands.literal("mute").executes(ctx -> {
                    client.setMute(null);
                    ctx.getSource().sendFeedback(msg("Toggled mute"));
                    return 1;
                }))
                .then(ClientCommands.literal("loop").executes(ctx -> {
                    client.cycleLoop();
                    ctx.getSource().sendFeedback(msg("Cycled loop mode"));
                    return 1;
                }))
                .then(ClientCommands.literal("shuffle").executes(ctx -> {
                    client.toggleShuffle();
                    ctx.getSource().sendFeedback(msg("Toggled shuffle"));
                    return 1;
                }))
                .then(ClientCommands.literal("fav").executes(ctx -> {
                    client.toggleFavorite(null);
                    ctx.getSource().sendFeedback(msg("Toggled favorite on current track"));
                    return 1;
                }))
                .then(ClientCommands.literal("clearqueue").executes(ctx -> {
                    client.clearQueue();
                    ctx.getSource().sendFeedback(msg("Cleared playback queue"));
                    return 1;
                }))
                .then(ClientCommands.literal("rescan").executes(ctx -> {
                    client.rescan();
                    ctx.getSource().sendFeedback(msg("Rescanning tntron library..."));
                    return 1;
                }))
                .then(ClientCommands.literal("reconnect").executes(ctx -> {
                    client.reconnect();
                    ctx.getSource().sendFeedback(msg("Reconnecting to tntron..."));
                    return 1;
                }))
                .then(ClientCommands.literal("vol")
                        .then(ClientCommands.argument("value", IntegerArgumentType.integer(0, 100))
                                .executes(ctx -> {
                                    int v = IntegerArgumentType.getInteger(ctx, "value");
                                    client.setVolume(v);
                                    ctx.getSource().sendFeedback(msg("Volume set to " + v + "%"));
                                    return 1;
                                })))
                .then(ClientCommands.literal("seek")
                        .then(ClientCommands.argument("seconds", DoubleArgumentType.doubleArg(0.0))
                                .executes(ctx -> {
                                    double s = DoubleArgumentType.getDouble(ctx, "seconds");
                                    client.seekAbsolute(s);
                                    ctx.getSource().sendFeedback(msg("Seeked to " + TntronThemeColors.formatTime(s)));
                                    return 1;
                                })))
                .then(ClientCommands.literal("speed")
                        .then(ClientCommands.argument("mult", DoubleArgumentType.doubleArg(0.25, 3.0))
                                .executes(ctx -> {
                                    double sp = DoubleArgumentType.getDouble(ctx, "mult");
                                    client.setSpeed(sp);
                                    ctx.getSource().sendFeedback(msg(String.format("Speed set to %.2fx", sp)));
                                    return 1;
                                })))
                .then(ClientCommands.literal("pitch")
                        .then(ClientCommands.argument("semitones", DoubleArgumentType.doubleArg(-12.0, 12.0))
                                .executes(ctx -> {
                                    double st = DoubleArgumentType.getDouble(ctx, "semitones");
                                    client.setPitch(st, null, null, null);
                                    ctx.getSource().sendFeedback(msg(String.format("Pitch set to %+.1f st", st)));
                                    return 1;
                                })))
                .then(ClientCommands.literal("dsp")
                        .then(ClientCommands.argument("preset", StringArgumentType.word())
                                .executes(ctx -> {
                                    String p = StringArgumentType.getString(ctx, "preset");
                                    client.setDspPreset(p);
                                    ctx.getSource().sendFeedback(msg("DSP preset: " + p));
                                    return 1;
                                })))
                .then(ClientCommands.literal("spatial").executes(ctx -> {
                    client.toggleSpatial();
                    ctx.getSource().sendFeedback(msg("Toggled 8D Spatial audio"));
                    return 1;
                }))
                .then(ClientCommands.literal("ab")
                        .then(ClientCommands.argument("action", StringArgumentType.word())
                                .executes(ctx -> {
                                    String act = StringArgumentType.getString(ctx, "action");
                                    client.abLoop(act);
                                    ctx.getSource().sendFeedback(msg("A-B Loop: " + act));
                                    return 1;
                                })))
                .then(ClientCommands.literal("sleep")
                        .then(ClientCommands.literal("off").executes(ctx -> {
                            client.setSleepMinutes(null);
                            ctx.getSource().sendFeedback(msg("Sleep timer cleared"));
                            return 1;
                        }))
                        .then(ClientCommands.argument("minutes", IntegerArgumentType.integer(1, 360))
                                .executes(ctx -> {
                                    int m = IntegerArgumentType.getInteger(ctx, "minutes");
                                    client.setSleepMinutes(m);
                                    ctx.getSource().sendFeedback(msg("Sleep timer set to " + m + "m"));
                                    return 1;
                                })))
                .then(ClientCommands.literal("theme")
                        .then(ClientCommands.argument("slug", StringArgumentType.word())
                                .executes(ctx -> {
                                    String slug = StringArgumentType.getString(ctx, "slug");
                                    client.setTheme(slug);
                                    ctx.getSource().sendFeedback(msg("Theme switched to " + slug));
                                    return 1;
                                })))
                .then(ClientCommands.literal("hud").executes(ctx -> {
                    config.hudEnabled = !config.hudEnabled;
                    config.save();
                    ctx.getSource().sendFeedback(msg("HUD Overlay: " + (config.hudEnabled ? "ON" : "OFF")));
                    return 1;
                }))
                .then(ClientCommands.literal("lyrics").executes(ctx -> {
                    config.lyricsOverlayEnabled = !config.lyricsOverlayEnabled;
                    config.save();
                    ctx.getSource().sendFeedback(msg("Synced Lyrics Overlay: " + (config.lyricsOverlayEnabled ? "ON" : "OFF")));
                    return 1;
                }))
                .then(ClientCommands.literal("reactive").executes(ctx -> {
                    config.reactiveEnabled = !config.reactiveEnabled;
                    config.save();
                    ctx.getSource().sendFeedback(msg("Beat-Reactive Effects: " + (config.reactiveEnabled ? "ON" : "OFF")));
                    return 1;
                }))
                .then(ClientCommands.literal("playpath")
                        .then(ClientCommands.argument("path", StringArgumentType.greedyString())
                                .executes(ctx -> {
                                    String path = StringArgumentType.getString(ctx, "path");
                                    client.playPath(path);
                                    ctx.getSource().sendFeedback(msg("Playing: " + path));
                                    return 1;
                                })))
                .then(ClientCommands.literal("queue")
                        .then(ClientCommands.argument("path", StringArgumentType.greedyString())
                                .executes(ctx -> {
                                    String path = StringArgumentType.getString(ctx, "path");
                                    client.addToQueue(path);
                                    ctx.getSource().sendFeedback(msg("Queued: " + path));
                                    return 1;
                                })))
                .then(ClientCommands.literal("search")
                        .then(ClientCommands.argument("query", StringArgumentType.greedyString())
                                .executes(ctx -> {
                                    String q = StringArgumentType.getString(ctx, "query");
                                    FabricClientCommandSource src = ctx.getSource();
                                    client.queryLibrary(q, null, 8, lib -> {
                                        if (lib == null || lib.tracks.isEmpty()) {
                                            src.sendFeedback(msg("No tracks found for '" + q + "'"));
                                            return;
                                        }
                                        src.sendFeedback(msg("Found " + lib.tracks.size() + " tracks (click to play):"));
                                        for (LibraryTrack tr : lib.tracks) {
                                            MutableComponent line = Component.literal("  ▶ " + tr.title)
                                                    .withStyle(ChatFormatting.GREEN)
                                                    .withStyle(s -> s.withClickEvent(new ClickEvent.RunCommand("/tntron playpath " + tr.path)));
                                            if (!tr.artist.isEmpty()) {
                                                line.append(Component.literal(" — " + tr.artist).withStyle(ChatFormatting.GRAY));
                                            }
                                            src.sendFeedback(line);
                                        }
                                    });
                                    return 1;
                                })))
                .then(ClientCommands.literal("status").executes(ctx -> {
                    TntronStatus st = client.getStatus();
                    if (!client.isConnected()) {
                        ctx.getSource().sendError(msg("Disconnected (" + client.getLastError() + ")"));
                        return 0;
                    }
                    String summary = String.format(
                            "%s · %s — %s [%s / %s] · Vol %.0f%% · %.2fx · DSP: %s · Loop: %s",
                            st.state.toUpperCase(),
                            st.artist.isEmpty() ? "tntron" : st.artist,
                            st.title,
                            TntronThemeColors.formatTime(st.position),
                            TntronThemeColors.formatTime(st.duration),
                            st.volume,
                            st.speed,
                            st.dsp_preset,
                            st.loop_mode
                    );
                    ctx.getSource().sendFeedback(msg(summary));
                    return 1;
                }))
        );
    }

    private static Component msg(String text) {
        return Component.literal("[tntron] ").withStyle(ChatFormatting.GREEN)
                .append(Component.literal(text).withStyle(ChatFormatting.WHITE));
    }
}
