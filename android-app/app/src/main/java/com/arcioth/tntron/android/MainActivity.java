package com.arcioth.tntron.android;

import android.content.SharedPreferences;
import android.graphics.Bitmap;
import android.graphics.BitmapFactory;
import android.graphics.Color;
import android.graphics.Typeface;
import android.graphics.drawable.GradientDrawable;
import android.os.Bundle;
import android.text.InputType;
import android.text.TextUtils;
import android.util.Base64;
import android.view.Gravity;
import android.view.View;
import android.view.ViewGroup;
import android.widget.Button;
import android.widget.EditText;
import android.widget.FrameLayout;
import android.widget.HorizontalScrollView;
import android.widget.ImageView;
import android.widget.LinearLayout;
import android.widget.ScrollView;
import android.widget.SeekBar;
import android.widget.TextView;
import android.widget.Toast;

import androidx.appcompat.app.AppCompatActivity;

import org.json.JSONArray;
import org.json.JSONObject;

public final class MainActivity extends AppCompatActivity implements TntronRemoteClient.Listener {
    private static final String PREFS = "tntron_remote_prefs";

    private TntronRemoteClient client;
    private SharedPreferences prefs;

    private int accentColor = 0xFF1DB954;
    private int activeTab = 0; // 0=Player, 1=DSP, 2=Library, 3=Audio/BT, 4=Connect
    private int libMode = 0;   // 0=Tracks, 1=Folders, 2=Queue

    private JSONObject lastStatus = new JSONObject();
    private JSONObject lastAndroidStatus = new JSONObject();
    private JSONObject lastLibrary = new JSONObject();
    private JSONArray lastThemes = new JSONArray();
    private JSONArray lastExtensions = new JSONArray();

    private String lastCoverTrackPath = "";
    private Bitmap coverBitmap;
    private boolean userSeeking = false;
    private boolean userVolDragging = false;
    private boolean userSpeedDragging = false;

    // Header
    private TextView headerBrand;
    private TextView headerStatusPill;
    private final Button[] navButtons = new Button[5];
    private final View[] tabPages = new View[5];

    // Tab 0: Player
    private ImageView coverView;
    private SpectrumView spectrumView;
    private TextView titleView;
    private TextView artistView;
    private TextView metaView;
    private SeekBar seekBar;
    private TextView timeLeftView;
    private TextView timeRightView;
    private Button playPauseBtn;
    private Button shuffleBtn;
    private Button loopBtn;
    private Button favBtn;
    private SeekBar volBar;
    private TextView volLabel;
    private Button muteBtn;
    private TextView lyricCurrView;
    private TextView lyricWindowView;

    // Tab 1: DSP
    private TextView dspSummaryView;
    private SeekBar speedBar;
    private TextView speedLabel;
    private TextView pitchLabel;
    private TextView eqLabel;
    private TextView abLabel;
    private TextView sleepLabel;

    // Tab 2: Library
    private EditText searchInput;
    private LinearLayout libraryListContainer;

    // Tab 3: Audio & BT
    private TextView audioSourceSummary;
    private TextView avrcpTitleView;
    private TextView avrcpSubView;
    private LinearLayout btDevicesContainer;
    private LinearLayout sinksContainer;
    private SeekBar btVolBar;
    private TextView btVolLabel;

    // Tab 4: Connect & Themes
    private EditText hostInput;
    private EditText portInput;
    private EditText tokenInput;
    private TextView connDetailView;
    private LinearLayout themesContainer;
    private LinearLayout extensionsContainer;

    @Override
    protected void onCreate(Bundle savedInstanceState) {
        super.onCreate(savedInstanceState);
        prefs = getSharedPreferences(PREFS, MODE_PRIVATE);
        client = new TntronRemoteClient(this);

        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setBackgroundColor(0xFF0A0D14);

        root.addView(buildHeader());

        FrameLayout content = new FrameLayout(this);
        LinearLayout.LayoutParams contentLp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f);
        root.addView(content, contentLp);

        tabPages[0] = buildPlayerTab();
        tabPages[1] = buildDspTab();
        tabPages[2] = buildLibraryTab();
        tabPages[3] = buildAudioBtTab();
        tabPages[4] = buildConnectTab();
        for (View page : tabPages) {
            content.addView(page, new FrameLayout.LayoutParams(
                    ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.MATCH_PARENT));
        }

        root.addView(buildBottomNav());
        setContentView(root);
        selectTab(0);

        String savedHost = prefs.getString("host", "127.0.0.1");
        int savedPort = prefs.getInt("port", 26263);
        String savedToken = prefs.getString("token", "");
        hostInput.setText(savedHost);
        portInput.setText(String.valueOf(savedPort));
        tokenInput.setText(savedToken);
        client.configureAndConnect(savedHost, savedPort, savedToken);
    }

    @Override
    protected void onDestroy() {
        super.onDestroy();
        if (client != null) client.shutdown();
    }

    private int dp(int v) {
        return Math.round(v * getResources().getDisplayMetrics().density);
    }

    private GradientDrawable roundedBg(int fillColor, int strokeColor, int radiusDp) {
        GradientDrawable d = new GradientDrawable();
        d.setColor(fillColor);
        d.setCornerRadius(dp(radiusDp));
        if (strokeColor != 0) d.setStroke(dp(1), strokeColor);
        return d;
    }

    private LinearLayout makeCard() {
        LinearLayout card = new LinearLayout(this);
        card.setOrientation(LinearLayout.VERTICAL);
        card.setBackground(roundedBg(0xFF131826, 0x22FFFFFF, 14));
        card.setPadding(dp(14), dp(12), dp(14), dp(12));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.bottomMargin = dp(10);
        card.setLayoutParams(lp);
        return card;
    }

    private TextView makeSectionHeading(String text) {
        TextView tv = new TextView(this);
        tv.setText(text);
        tv.setTextColor(0xFF94A3B8);
        tv.setTextSize(12f);
        tv.setTypeface(Typeface.DEFAULT_BOLD);
        tv.setAllCaps(true);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.bottomMargin = dp(6);
        tv.setLayoutParams(lp);
        return tv;
    }

    private Button makeBtn(String label, boolean primary, View.OnClickListener onClick) {
        Button b = new Button(this);
        b.setText(label);
        b.setAllCaps(false);
        b.setTextSize(13f);
        b.setTextColor(primary ? 0xFF06100A : 0xFFF1F5F9);
        b.setBackground(roundedBg(primary ? accentColor : 0xFF1D2436, 0x22FFFFFF, 10));
        b.setPadding(dp(10), dp(6), dp(10), dp(6));
        b.setMinimumHeight(dp(38));
        b.setMinHeight(dp(38));
        b.setMinWidth(0);
        b.setMinimumWidth(0);
        b.setOnClickListener(onClick);
        return b;
    }

    private LinearLayout makeRow() {
        LinearLayout row = new LinearLayout(this);
        row.setOrientation(LinearLayout.HORIZONTAL);
        row.setGravity(Gravity.CENTER_VERTICAL);
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.topMargin = dp(4);
        lp.bottomMargin = dp(4);
        row.setLayoutParams(lp);
        return row;
    }

    private void addWeightedBtn(LinearLayout row, Button b, float weight) {
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, weight);
        lp.leftMargin = dp(3);
        lp.rightMargin = dp(3);
        row.addView(b, lp);
    }

    private View buildHeader() {
        LinearLayout bar = new LinearLayout(this);
        bar.setOrientation(LinearLayout.HORIZONTAL);
        bar.setGravity(Gravity.CENTER_VERTICAL);
        bar.setBackgroundColor(0xFF101521);
        bar.setPadding(dp(14), dp(12), dp(14), dp(10));

        headerBrand = new TextView(this);
        headerBrand.setText("tntron");
        headerBrand.setTextColor(accentColor);
        headerBrand.setTextSize(19f);
        headerBrand.setTypeface(Typeface.DEFAULT_BOLD);
        bar.addView(headerBrand, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

        headerStatusPill = new TextView(this);
        headerStatusPill.setText("Connecting...");
        headerStatusPill.setTextColor(0xFFF8FAFC);
        headerStatusPill.setTextSize(11f);
        headerStatusPill.setPadding(dp(10), dp(4), dp(10), dp(4));
        headerStatusPill.setBackground(roundedBg(0xFF1F2937, 0x33FFFFFF, 20));
        headerStatusPill.setOnClickListener(v -> selectTab(4));
        bar.addView(headerStatusPill);
        return bar;
    }

    private View buildBottomNav() {
        LinearLayout nav = new LinearLayout(this);
        nav.setOrientation(LinearLayout.HORIZONTAL);
        nav.setBackgroundColor(0xFF101521);
        nav.setPadding(dp(4), dp(6), dp(4), dp(8));
        String[] labels = {"Player", "DSP", "Library", "Audio/BT", "Connect"};
        for (int i = 0; i < 5; i++) {
            final int idx = i;
            Button b = makeBtn(labels[i], false, v -> selectTab(idx));
            b.setTextSize(11f);
            navButtons[i] = b;
            addWeightedBtn(nav, b, 1f);
        }
        return nav;
    }

    private void selectTab(int idx) {
        activeTab = idx;
        for (int i = 0; i < tabPages.length; i++) {
            tabPages[i].setVisibility(i == idx ? View.VISIBLE : View.GONE);
            boolean active = (i == idx);
            navButtons[i].setBackground(roundedBg(active ? accentColor : 0xFF171D2D, 0, 10));
            navButtons[i].setTextColor(active ? 0xFF06100A : 0xFFCBD5E1);
        }
        if (idx == 2) refreshLibrary();
        else if (idx == 3) refreshAndroidStatus();
        else if (idx == 4) {
            refreshThemes();
            refreshExtensions();
        }
    }

    private View buildPlayerTab() {
        ScrollView sv = new ScrollView(this);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setPadding(dp(14), dp(12), dp(14), dp(14));
        sv.addView(col);

        // Cover Art + Track Info
        LinearLayout heroCard = makeCard();
        heroCard.setGravity(Gravity.CENTER_HORIZONTAL);
        coverView = new ImageView(this);
        coverView.setScaleType(ImageView.ScaleType.CENTER_CROP);
        coverView.setBackground(roundedBg(0xFF1E2433, 0x33FFFFFF, 16));
        coverView.setImageResource(R.mipmap.ic_launcher);
        LinearLayout.LayoutParams covLp = new LinearLayout.LayoutParams(dp(190), dp(190));
        covLp.bottomMargin = dp(12);
        heroCard.addView(coverView, covLp);

        titleView = new TextView(this);
        titleView.setText("No Active Track");
        titleView.setTextColor(0xFFF8FAFC);
        titleView.setTextSize(18f);
        titleView.setTypeface(Typeface.DEFAULT_BOLD);
        titleView.setGravity(Gravity.CENTER);
        titleView.setMaxLines(2);
        titleView.setEllipsize(TextUtils.TruncateAt.END);
        heroCard.addView(titleView);

        artistView = new TextView(this);
        artistView.setText("Connect to tntron on PC");
        artistView.setTextColor(0xFF94A3B8);
        artistView.setTextSize(13f);
        artistView.setGravity(Gravity.CENTER);
        artistView.setMaxLines(1);
        artistView.setEllipsize(TextUtils.TruncateAt.END);
        heroCard.addView(artistView);

        metaView = new TextView(this);
        metaView.setText("");
        metaView.setTextColor(accentColor);
        metaView.setTextSize(11f);
        metaView.setGravity(Gravity.CENTER);
        metaView.setPadding(0, dp(4), 0, dp(8));
        heroCard.addView(metaView);

        spectrumView = new SpectrumView(this);
        LinearLayout.LayoutParams spLp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, dp(52));
        spLp.bottomMargin = dp(8);
        heroCard.addView(spectrumView, spLp);

        // Seek bar
        seekBar = new SeekBar(this);
        seekBar.setMax(1000);
        seekBar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar sb, int p, boolean fromUser) {
                if (fromUser) {
                    double dur = lastStatus.optDouble("duration", 0.0);
                    timeLeftView.setText(ThemeColors.formatTime((p / 1000.0) * dur));
                }
            }
            @Override public void onStartTrackingTouch(SeekBar sb) { userSeeking = true; }
            @Override public void onStopTrackingTouch(SeekBar sb) {
                userSeeking = false;
                double dur = lastStatus.optDouble("duration", 0.0);
                double target = (sb.getProgress() / 1000.0) * dur;
                client.sendWith("seek", o -> { try { o.put("value", target); } catch (Exception ignored) {} }, null);
            }
        });
        heroCard.addView(seekBar, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT));

        LinearLayout timeRow = makeRow();
        timeLeftView = new TextView(this);
        timeLeftView.setText("0:00");
        timeLeftView.setTextColor(0xFF94A3B8);
        timeLeftView.setTextSize(11f);
        timeRightView = new TextView(this);
        timeRightView.setText("0:00");
        timeRightView.setTextColor(0xFF94A3B8);
        timeRightView.setTextSize(11f);
        timeRightView.setGravity(Gravity.END);
        timeRow.addView(timeLeftView, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        timeRow.addView(timeRightView, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        heroCard.addView(timeRow);

        // Primary transport
        LinearLayout tRow = makeRow();
        addWeightedBtn(tRow, makeBtn("-10s", false, v -> sendSeekDelta(-10)), 0.8f);
        addWeightedBtn(tRow, makeBtn("⏮", false, v -> client.sendCmd("prev")), 1f);
        playPauseBtn = makeBtn("▶ Play", true, v -> client.sendCmd("toggle"));
        addWeightedBtn(tRow, playPauseBtn, 1.5f);
        addWeightedBtn(tRow, makeBtn("⏭", false, v -> client.sendCmd("next")), 1f);
        addWeightedBtn(tRow, makeBtn("+10s", false, v -> sendSeekDelta(10)), 0.8f);
        heroCard.addView(tRow);

        // Secondary transport
        LinearLayout tRow2 = makeRow();
        shuffleBtn = makeBtn("🔀 Off", false, v -> client.sendCmd("shuffle"));
        loopBtn = makeBtn("🔁 All", false, v -> client.sendCmd("cycle_loop"));
        favBtn = makeBtn("♡ Fav", false, v -> client.sendCmd("toggle_favorite"));
        addWeightedBtn(tRow2, shuffleBtn, 1f);
        addWeightedBtn(tRow2, loopBtn, 1f);
        addWeightedBtn(tRow2, favBtn, 1f);
        addWeightedBtn(tRow2, makeBtn("■ Stop", false, v -> client.sendCmd("stop")), 0.9f);
        heroCard.addView(tRow2);
        col.addView(heroCard);

        // Volume card
        LinearLayout volCard = makeCard();
        LinearLayout vHead = makeRow();
        volLabel = new TextView(this);
        volLabel.setText("PC Volume: 100%");
        volLabel.setTextColor(0xFFCBD5E1);
        volLabel.setTextSize(13f);
        vHead.addView(volLabel, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        muteBtn = makeBtn("Mute", false, v -> client.sendCmd("mute"));
        vHead.addView(muteBtn);
        volCard.addView(vHead);

        volBar = new SeekBar(this);
        volBar.setMax(100);
        volBar.setProgress(100);
        volBar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar sb, int p, boolean fromUser) {
                if (fromUser) volLabel.setText("PC Volume: " + p + "%");
            }
            @Override public void onStartTrackingTouch(SeekBar sb) { userVolDragging = true; }
            @Override public void onStopTrackingTouch(SeekBar sb) {
                userVolDragging = false;
                int v = sb.getProgress();
                client.sendWith("volume", o -> { try { o.put("value", (double) v); } catch (Exception ignored) {} }, null);
            }
        });
        volCard.addView(volBar);
        col.addView(volCard);

        // Lyrics card
        LinearLayout lyrCard = makeCard();
        lyrCard.addView(makeSectionHeading("Live Synced Lyrics"));
        lyricCurrView = new TextView(this);
        lyricCurrView.setText("No synced lyrics");
        lyricCurrView.setTextColor(accentColor);
        lyricCurrView.setTextSize(15f);
        lyricCurrView.setTypeface(Typeface.DEFAULT_BOLD);
        lyrCard.addView(lyricCurrView);

        lyricWindowView = new TextView(this);
        lyricWindowView.setText("");
        lyricWindowView.setTextColor(0xFF94A3B8);
        lyricWindowView.setTextSize(12f);
        lyricWindowView.setPadding(0, dp(4), 0, 0);
        lyrCard.addView(lyricWindowView);
        col.addView(lyrCard);

        return sv;
    }

    private View buildDspTab() {
        ScrollView sv = new ScrollView(this);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setPadding(dp(14), dp(12), dp(14), dp(14));
        sv.addView(col);

        // Speed & Pitch
        LinearLayout spCard = makeCard();
        spCard.addView(makeSectionHeading("Speed & Pitch"));
        speedLabel = new TextView(this);
        speedLabel.setText("Speed: 1.00x");
        speedLabel.setTextColor(0xFFF1F5F9);
        spCard.addView(speedLabel);

        speedBar = new SeekBar(this);
        speedBar.setMax(55); // 0.25 .. 3.00 in 0.05 steps
        speedBar.setProgress(15);
        speedBar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar sb, int p, boolean fromUser) {
                if (fromUser) speedLabel.setText(String.format("Speed: %.2fx", 0.25 + p * 0.05));
            }
            @Override public void onStartTrackingTouch(SeekBar sb) { userSpeedDragging = true; }
            @Override public void onStopTrackingTouch(SeekBar sb) {
                userSpeedDragging = false;
                double sp = Math.round((0.25 + sb.getProgress() * 0.05) * 20.0) / 20.0;
                client.sendWith("speed", o -> { try { o.put("value", sp); } catch (Exception ignored) {} }, null);
            }
        });
        spCard.addView(speedBar);

        LinearLayout spPresets = makeRow();
        double[] speeds = {0.75, 0.85, 1.0, 1.25, 1.5};
        for (double s : speeds) {
            addWeightedBtn(spPresets, makeBtn(s + "x", false, v ->
                    client.sendWith("speed", o -> { try { o.put("value", s); } catch (Exception ignored) {} }, null)), 1f);
        }
        spCard.addView(spPresets);

        pitchLabel = new TextView(this);
        pitchLabel.setText("Pitch: +0.0 st");
        pitchLabel.setTextColor(0xFFCBD5E1);
        pitchLabel.setPadding(0, dp(8), 0, dp(2));
        spCard.addView(pitchLabel);

        LinearLayout pitchRow = makeRow();
        addWeightedBtn(pitchRow, makeBtn("-1 st", false, v -> sendPitch(null, -1.0, null, null)), 1f);
        addWeightedBtn(pitchRow, makeBtn("0 st", false, v -> sendPitch(0.0, null, null, null)), 1f);
        addWeightedBtn(pitchRow, makeBtn("+1 st", false, v -> sendPitch(null, 1.0, null, null)), 1f);
        addWeightedBtn(pitchRow, makeBtn("Link Pitch", false, v -> sendPitch(null, null, !lastStatus.optBoolean("link_pitch", false), null)), 1.3f);
        spCard.addView(pitchRow);
        col.addView(spCard);

        // DSP Presets & 3-Band EQ
        LinearLayout dspCard = makeCard();
        dspCard.addView(makeSectionHeading("DSP Presets & 3-Band EQ"));
        dspSummaryView = new TextView(this);
        dspSummaryView.setText("Active Preset: Flat");
        dspSummaryView.setTextColor(accentColor);
        dspCard.addView(dspSummaryView);

        String[][] presets = {{"flat", "Flat"}, {"bass", "Bass+"}, {"vocal", "Vocal"}, {"warm", "Warm"}, {"rock", "Rock"}, {"8d", "8D"}};
        LinearLayout pRow1 = makeRow();
        LinearLayout pRow2 = makeRow();
        for (int i = 0; i < presets.length; i++) {
            final String k = presets[i][0];
            addWeightedBtn(i < 3 ? pRow1 : pRow2, makeBtn(presets[i][1], false, v ->
                    client.sendWith("dsp_preset", o -> { try { o.put("preset", k); } catch (Exception ignored) {} }, null)), 1f);
        }
        dspCard.addView(pRow1);
        dspCard.addView(pRow2);

        eqLabel = new TextView(this);
        eqLabel.setText("Bass 0 dB · Mid 0 dB · Treble 0 dB");
        eqLabel.setTextColor(0xFFCBD5E1);
        eqLabel.setPadding(0, dp(8), 0, dp(4));
        dspCard.addView(eqLabel);

        LinearLayout eqRow = makeRow();
        addWeightedBtn(eqRow, makeBtn("Bass-", false, v -> sendEq("step_bass", -1)), 1f);
        addWeightedBtn(eqRow, makeBtn("Bass+", false, v -> sendEq("step_bass", 1)), 1f);
        addWeightedBtn(eqRow, makeBtn("Mid-", false, v -> sendEq("step_mid", -1)), 1f);
        addWeightedBtn(eqRow, makeBtn("Mid+", false, v -> sendEq("step_mid", 1)), 1f);
        addWeightedBtn(eqRow, makeBtn("Treb-", false, v -> sendEq("step_treble", -1)), 1f);
        addWeightedBtn(eqRow, makeBtn("Treb+", false, v -> sendEq("step_treble", 1)), 1f);
        dspCard.addView(eqRow);

        LinearLayout sp8d = makeRow();
        addWeightedBtn(sp8d, makeBtn("Toggle 8D Spatial", false, v -> client.sendCmd("toggle_spatial")), 2f);
        addWeightedBtn(sp8d, makeBtn("-Hz", false, v -> client.sendWith("set_spatial", o -> { try { o.put("delta_hz", -0.02); } catch (Exception ignored) {} }, null)), 1f);
        addWeightedBtn(sp8d, makeBtn("+Hz", false, v -> client.sendWith("set_spatial", o -> { try { o.put("delta_hz", 0.02); } catch (Exception ignored) {} }, null)), 1f);
        dspCard.addView(sp8d);
        col.addView(dspCard);

        // A-B Loop & Sleep Timer
        LinearLayout extraCard = makeCard();
        extraCard.addView(makeSectionHeading("A-B Loop & Sleep Timer"));
        abLabel = new TextView(this);
        abLabel.setText("A-B Loop: -- / --");
        abLabel.setTextColor(0xFFCBD5E1);
        extraCard.addView(abLabel);
        LinearLayout abRow = makeRow();
        addWeightedBtn(abRow, makeBtn("Mark A", false, v -> sendAb("mark_a")), 1f);
        addWeightedBtn(abRow, makeBtn("Mark B", false, v -> sendAb("mark_b")), 1f);
        addWeightedBtn(abRow, makeBtn("Clear", false, v -> sendAb("clear")), 1f);
        addWeightedBtn(abRow, makeBtn("Save", false, v -> sendAb("save")), 1f);
        extraCard.addView(abRow);

        sleepLabel = new TextView(this);
        sleepLabel.setText("Sleep Timer: Off");
        sleepLabel.setTextColor(0xFFCBD5E1);
        sleepLabel.setPadding(0, dp(8), 0, dp(2));
        extraCard.addView(sleepLabel);
        LinearLayout slpRow = makeRow();
        addWeightedBtn(slpRow, makeBtn("Off", false, v -> sendSleep(null)), 1f);
        addWeightedBtn(slpRow, makeBtn("15m", false, v -> sendSleep(15)), 1f);
        addWeightedBtn(slpRow, makeBtn("30m", false, v -> sendSleep(30)), 1f);
        addWeightedBtn(slpRow, makeBtn("60m", false, v -> sendSleep(60)), 1f);
        extraCard.addView(slpRow);
        col.addView(extraCard);

        return sv;
    }

    private View buildLibraryTab() {
        LinearLayout root = new LinearLayout(this);
        root.setOrientation(LinearLayout.VERTICAL);
        root.setPadding(dp(12), dp(10), dp(12), dp(10));

        LinearLayout searchRow = makeRow();
        searchInput = new EditText(this);
        searchInput.setHint("Search PC library...");
        searchInput.setHintTextColor(0xFF64748B);
        searchInput.setTextColor(0xFFF8FAFC);
        searchInput.setBackground(roundedBg(0xFF171D2D, 0x33FFFFFF, 10));
        searchInput.setPadding(dp(10), dp(8), dp(10), dp(8));
        searchInput.setSingleLine(true);
        searchRow.addView(searchInput, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        Button searchBtn = makeBtn("Search", true, v -> {
            libMode = 0;
            refreshLibrary();
        });
        LinearLayout.LayoutParams sblp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.WRAP_CONTENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        sblp.leftMargin = dp(6);
        searchRow.addView(searchBtn, sblp);
        root.addView(searchRow);

        LinearLayout modeRow = makeRow();
        addWeightedBtn(modeRow, makeBtn("Tracks", false, v -> { libMode = 0; renderLibraryList(); }), 1f);
        addWeightedBtn(modeRow, makeBtn("Folders", false, v -> { libMode = 1; renderLibraryList(); }), 1f);
        addWeightedBtn(modeRow, makeBtn("Queue", false, v -> { libMode = 2; renderLibraryList(); }), 1f);
        addWeightedBtn(modeRow, makeBtn("♥ Favs", false, v -> client.sendWith("play_favorites", o -> { try { o.put("shuffle", false); } catch (Exception ignored) {} }, null)), 1f);
        addWeightedBtn(modeRow, makeBtn("Rescan", false, v -> { client.sendCmd("rescan"); refreshLibrary(); }), 1f);
        root.addView(modeRow);

        ScrollView sv = new ScrollView(this);
        libraryListContainer = new LinearLayout(this);
        libraryListContainer.setOrientation(LinearLayout.VERTICAL);
        sv.addView(libraryListContainer);
        root.addView(sv, new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, 0, 1f));
        return root;
    }

    private View buildAudioBtTab() {
        ScrollView sv = new ScrollView(this);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setPadding(dp(14), dp(12), dp(14), dp(14));
        sv.addView(col);

        // Source Switcher
        LinearLayout srcCard = makeCard();
        srcCard.addView(makeSectionHeading("Active Audio Source (PC vs Reverse Bluetooth)"));
        audioSourceSummary = new TextView(this);
        audioSourceSummary.setText("Active Source: PC (tntron)");
        audioSourceSummary.setTextColor(accentColor);
        audioSourceSummary.setTextSize(14f);
        audioSourceSummary.setTypeface(Typeface.DEFAULT_BOLD);
        srcCard.addView(audioSourceSummary);

        LinearLayout srcRow = makeRow();
        addWeightedBtn(srcRow, makeBtn("PC (tntron)", true, v -> switchSource("pc")), 1f);
        addWeightedBtn(srcRow, makeBtn("Android (Reverse BT)", false, v -> switchSource("android")), 1.2f);
        addWeightedBtn(srcRow, makeBtn("Refresh", false, v -> refreshAndroidStatus()), 0.8f);
        srcCard.addView(srcRow);
        col.addView(srcCard);

        // Android AVRCP Control + Reverse BT Volume
        LinearLayout avCard = makeCard();
        avCard.addView(makeSectionHeading("Bluetooth Phone Playback (AVRCP)"));
        avrcpTitleView = new TextView(this);
        avrcpTitleView.setText("No active Bluetooth media player");
        avrcpTitleView.setTextColor(0xFFF8FAFC);
        avrcpTitleView.setTextSize(14f);
        avrcpTitleView.setTypeface(Typeface.DEFAULT_BOLD);
        avCard.addView(avrcpTitleView);

        avrcpSubView = new TextView(this);
        avrcpSubView.setText("Pair your phone with PC Bluetooth to stream & control");
        avrcpSubView.setTextColor(0xFF94A3B8);
        avrcpSubView.setTextSize(12f);
        avCard.addView(avrcpSubView);

        LinearLayout avRow = makeRow();
        addWeightedBtn(avRow, makeBtn("⏮", false, v -> sendAndroidMedia("prev")), 1f);
        addWeightedBtn(avRow, makeBtn("⏯ Play/Pause", true, v -> sendAndroidMedia("toggle")), 1.5f);
        addWeightedBtn(avRow, makeBtn("⏭", false, v -> sendAndroidMedia("next")), 1f);
        addWeightedBtn(avRow, makeBtn("■", false, v -> sendAndroidMedia("stop")), 0.8f);
        avCard.addView(avRow);

        btVolLabel = new TextView(this);
        btVolLabel.setText("Reverse BT Volume (Phone -> PC): 100%");
        btVolLabel.setTextColor(0xFFCBD5E1);
        btVolLabel.setPadding(0, dp(8), 0, dp(2));
        avCard.addView(btVolLabel);

        btVolBar = new SeekBar(this);
        btVolBar.setMax(150);
        btVolBar.setProgress(100);
        btVolBar.setOnSeekBarChangeListener(new SeekBar.OnSeekBarChangeListener() {
            @Override public void onProgressChanged(SeekBar sb, int p, boolean fromUser) {
                if (fromUser) btVolLabel.setText("Reverse BT Volume (Phone -> PC): " + p + "%");
            }
            @Override public void onStartTrackingTouch(SeekBar sb) {}
            @Override public void onStopTrackingTouch(SeekBar sb) {
                int pct = sb.getProgress();
                client.sendWith("bt_stream_volume", o -> { try { o.put("volume_pct", pct); } catch (Exception ignored) {} }, null);
            }
        });
        avCard.addView(btVolBar);
        col.addView(avCard);

        // PipeWire Sinks
        LinearLayout sinkCard = makeCard();
        sinkCard.addView(makeSectionHeading("PC Audio Output Devices (PipeWire)"));
        sinksContainer = new LinearLayout(this);
        sinksContainer.setOrientation(LinearLayout.VERTICAL);
        sinkCard.addView(sinksContainer);
        col.addView(sinkCard);

        // Bluetooth Devices
        LinearLayout btCard = makeCard();
        btCard.addView(makeSectionHeading("PC Bluetooth Devices (Reverse A2DP)"));
        btDevicesContainer = new LinearLayout(this);
        btDevicesContainer.setOrientation(LinearLayout.VERTICAL);
        btCard.addView(btDevicesContainer);
        col.addView(btCard);

        return sv;
    }

    private View buildConnectTab() {
        ScrollView sv = new ScrollView(this);
        LinearLayout col = new LinearLayout(this);
        col.setOrientation(LinearLayout.VERTICAL);
        col.setPadding(dp(14), dp(12), dp(14), dp(14));
        sv.addView(col);

        // Connection setup
        LinearLayout connCard = makeCard();
        connCard.addView(makeSectionHeading("Connect to tntron on PC"));
        connDetailView = new TextView(this);
        connDetailView.setText("Enable 'Android & Bluetooth Link' in tntron to view PC LAN IP & port (26263).");
        connDetailView.setTextColor(0xFF94A3B8);
        connDetailView.setTextSize(12f);
        connDetailView.setPadding(0, 0, 0, dp(8));
        connCard.addView(connDetailView);

        hostInput = makeInput("PC IP Address (e.g. 192.168.1.42 or 127.0.0.1)", InputType.TYPE_CLASS_TEXT);
        portInput = makeInput("Port (26263)", InputType.TYPE_CLASS_NUMBER);
        tokenInput = makeInput("Optional Pairing PIN / Token", InputType.TYPE_CLASS_TEXT);
        connCard.addView(hostInput);
        connCard.addView(portInput);
        connCard.addView(tokenInput);

        LinearLayout btnRow = makeRow();
        addWeightedBtn(btnRow, makeBtn("Connect", true, v -> applyConnection()), 1.4f);
        addWeightedBtn(btnRow, makeBtn("USB (127.0.0.1)", false, v -> {
            hostInput.setText("127.0.0.1");
            portInput.setText("26263");
            applyConnection();
        }), 1.2f);
        connCard.addView(btnRow);
        col.addView(connCard);

        // Themes
        LinearLayout themeCard = makeCard();
        LinearLayout thHead = makeRow();
        TextView thLbl = makeSectionHeading("tntron Themes");
        thHead.addView(thLbl, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
        thHead.addView(makeBtn("Toggle Hide-UI", false, v -> client.sendCmd("toggle_hide_ui")));
        themeCard.addView(thHead);
        themesContainer = new LinearLayout(this);
        themesContainer.setOrientation(LinearLayout.VERTICAL);
        themeCard.addView(themesContainer);
        col.addView(themeCard);

        // Extensions
        LinearLayout extCard = makeCard();
        extCard.addView(makeSectionHeading("tntron Extensions"));
        extensionsContainer = new LinearLayout(this);
        extensionsContainer.setOrientation(LinearLayout.VERTICAL);
        extCard.addView(extensionsContainer);
        col.addView(extCard);

        return sv;
    }

    private EditText makeInput(String hint, int inputType) {
        EditText et = new EditText(this);
        et.setHint(hint);
        et.setHintTextColor(0xFF64748B);
        et.setTextColor(0xFFF8FAFC);
        et.setInputType(inputType);
        et.setSingleLine(true);
        et.setBackground(roundedBg(0xFF1A2233, 0x33FFFFFF, 10));
        et.setPadding(dp(12), dp(10), dp(12), dp(10));
        LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
        lp.bottomMargin = dp(8);
        et.setLayoutParams(lp);
        return et;
    }

    private void applyConnection() {
        String h = hostInput.getText().toString().trim();
        if (h.isEmpty()) h = "127.0.0.1";
        int p = 26263;
        try { p = Integer.parseInt(portInput.getText().toString().trim()); } catch (Exception ignored) {}
        String tok = tokenInput.getText().toString().trim();
        prefs.edit().putString("host", h).putInt("port", p).putString("token", tok).apply();
        client.configureAndConnect(h, p, tok);
        Toast.makeText(this, "Connecting to " + h + ":" + p, Toast.LENGTH_SHORT).show();
    }

    @Override
    public void onConnectionChanged(boolean connected, String message) {
        headerStatusPill.setText(connected ? "● Connected" : "○ Offline");
        headerStatusPill.setTextColor(connected ? 0xFF4ADE80 : 0xFFF87171);
        if (connDetailView != null) connDetailView.setText(message);
        if (connected) {
            refreshAndroidStatus();
            refreshThemes();
            refreshExtensions();
        }
    }

    @Override
    public void onStatus(JSONObject st) {
        lastStatus = st;
        accentColor = ThemeColors.resolveAccent(st.optString("active_theme", "openify"));
        headerBrand.setTextColor(accentColor);

        boolean playing = "playing".equalsIgnoreCase(st.optString("state", "stopped"));
        String title = st.optString("title", "No Active Track");
        String artist = st.optString("artist", "");
        String album = st.optString("album", "");
        String trackPath = st.optString("track_path", "");

        titleView.setText(title.isEmpty() ? "No Active Track" : title);
        artistView.setText(artist.isEmpty() ? "tntron" : (album.isEmpty() ? artist : artist + " · " + album));

        StringBuilder meta = new StringBuilder();
        String codec = st.optString("codec", "");
        if (!codec.isEmpty() && !"null".equals(codec)) meta.append(codec.toUpperCase());
        if (st.has("bpm") && !st.isNull("bpm")) {
            if (meta.length() > 0) meta.append(" · ");
            meta.append(Math.round(st.optDouble("bpm"))).append(" BPM");
        }
        double speed = st.optDouble("speed", 1.0);
        if (meta.length() > 0) meta.append(" · ");
        meta.append(String.format("%.2fx · %s", speed, st.optString("dsp_preset", "Flat")));
        metaView.setText(meta.toString());
        metaView.setTextColor(accentColor);

        // Spectrum
        float[] bands = new float[16];
        JSONArray bArr = st.optJSONArray("bands");
        if (bArr != null) {
            for (int i = 0; i < Math.min(16, bArr.length()); i++) {
                bands[i] = (float) bArr.optDouble(i, 0.0);
            }
        }
        spectrumView.updateSpectrum(bands, accentColor, playing);

        // Progress
        double pos = st.optDouble("position", 0.0);
        double dur = st.optDouble("duration", 0.0);
        if (!userSeeking) {
            timeLeftView.setText(ThemeColors.formatTime(pos));
            timeRightView.setText(ThemeColors.formatTime(dur));
            seekBar.setProgress(dur > 0 ? (int) Math.round((pos / dur) * 1000.0) : 0);
        }

        playPauseBtn.setText(playing ? "⏸ Pause" : "▶ Play");
        playPauseBtn.setBackground(roundedBg(accentColor, 0, 10));
        shuffleBtn.setText("🔀 " + (st.optBoolean("shuffle", false) ? "ON" : "Off"));
        loopBtn.setText("🔁 " + st.optString("loop_mode", "All"));
        favBtn.setText(st.optBoolean("is_favorite", false) ? "♥ Fav" : "♡ Fav");

        if (!userVolDragging) {
            int vol = (int) Math.round(st.optDouble("volume", 100.0));
            volBar.setProgress(vol);
            volLabel.setText("PC Volume: " + vol + "%" + (st.optBoolean("muted", false) ? " (Muted)" : ""));
        }
        muteBtn.setText(st.optBoolean("muted", false) ? "Unmute" : "Mute");

        // Lyrics
        String currLyric = st.optString("lyric", "");
        lyricCurrView.setText(currLyric.isEmpty() ? "No synced lyrics for current track" : currLyric);
        lyricCurrView.setTextColor(currLyric.isEmpty() ? 0xFF64748B : accentColor);
        JSONArray lw = st.optJSONArray("lyrics_window");
        if (lw != null && lw.length() > 0) {
            StringBuilder sb = new StringBuilder();
            for (int i = 0; i < lw.length(); i++) {
                if (i > 0) sb.append('\n');
                sb.append(lw.optString(i, ""));
            }
            lyricWindowView.setText(sb.toString());
        } else {
            lyricWindowView.setText("");
        }

        // DSP labels
        if (!userSpeedDragging) {
            speedLabel.setText(String.format("Speed: %.2fx", speed));
            speedBar.setProgress(Math.max(0, Math.min(55, (int) Math.round((speed - 0.25) / 0.05))));
        }
        pitchLabel.setText(String.format("Pitch: %+.1f st · Link: %s",
                st.optDouble("pitch_st", 0.0), st.optBoolean("link_pitch", false) ? "ON" : "OFF"));
        dspSummaryView.setText("Active Preset: " + st.optString("dsp_preset", "Flat")
                + (st.optBoolean("spatial_on", false) ? String.format(" · 8D ON (%.2f Hz)", st.optDouble("spatial_hz", 0.08)) : ""));
        eqLabel.setText(String.format("Bass %+d dB · Mid %+d dB · Treble %+d dB",
                st.optInt("bass_db", 0), st.optInt("mid_db", 0), st.optInt("treble_db", 0)));
        String aStr = st.isNull("ab_loop_a") ? "--" : ThemeColors.formatTime(st.optDouble("ab_loop_a"));
        String bStr = st.isNull("ab_loop_b") ? "--" : ThemeColors.formatTime(st.optDouble("ab_loop_b"));
        abLabel.setText("A-B Loop: A=" + aStr + "  B=" + bStr);
        sleepLabel.setText(st.isNull("sleep_remaining_sec")
                ? "Sleep Timer: Off"
                : ("Sleep Timer: " + ThemeColors.formatTime(st.optDouble("sleep_remaining_sec")) + " left"));

        // Fetch cover art on track change
        if (!trackPath.equals(lastCoverTrackPath)) {
            lastCoverTrackPath = trackPath;
            if (!trackPath.isEmpty()) {
                client.sendWith("get_cover", null, resp -> {
                    if (resp != null && !resp.isNull("cover_base64")) {
                        String b64 = resp.optString("cover_base64", "");
                        if (!b64.isEmpty()) {
                            try {
                                byte[] bytes = Base64.decode(b64, Base64.DEFAULT);
                                coverBitmap = BitmapFactory.decodeByteArray(bytes, 0, bytes.length);
                                if (coverBitmap != null) {
                                    coverView.setImageBitmap(coverBitmap);
                                    return;
                                }
                            } catch (Exception ignored) {}
                        }
                    }
                    coverView.setImageResource(R.mipmap.ic_launcher);
                });
            } else {
                coverView.setImageResource(R.mipmap.ic_launcher);
            }
        }
    }

    private void sendSeekDelta(long delta) {
        client.sendWith("seek", o -> { try { o.put("delta", delta); } catch (Exception ignored) {} }, null);
    }

    private void sendPitch(Double value, Double delta, Boolean link, Boolean formant) {
        client.sendWith("pitch", o -> {
            try {
                if (value != null) o.put("value", value);
                if (delta != null) o.put("delta", delta);
                if (link != null) o.put("link_pitch", link);
                if (formant != null) o.put("preserve_formant", formant);
            } catch (Exception ignored) {}
        }, null);
    }

    private void sendEq(String cmd, int delta) {
        client.sendWith(cmd, o -> { try { o.put("delta", delta); } catch (Exception ignored) {} }, null);
    }

    private void sendAb(String action) {
        client.sendWith("ab_loop", o -> { try { o.put("action", action); } catch (Exception ignored) {} }, null);
    }

    private void sendSleep(Integer mins) {
        client.sendWith("sleep", o -> {
            try {
                if (mins != null) o.put("minutes", mins);
                else o.put("minutes", JSONObject.NULL);
            } catch (Exception ignored) {}
        }, null);
    }

    private void switchSource(String src) {
        client.sendWith("switch_audio_source", o -> { try { o.put("source", src); } catch (Exception ignored) {} },
                r -> refreshAndroidStatus());
    }

    private void sendAndroidMedia(String action) {
        client.sendWith("android_media", o -> { try { o.put("action", action); } catch (Exception ignored) {} },
                r -> refreshAndroidStatus());
    }

    private void refreshLibrary() {
        String q = searchInput != null ? searchInput.getText().toString().trim() : "";
        client.sendWith("get_library", o -> {
            try {
                if (!q.isEmpty()) o.put("query", q);
                o.put("limit", 80);
            } catch (Exception ignored) {}
        }, resp -> {
            if (resp != null && resp.has("library") && !resp.isNull("library")) {
                lastLibrary = resp.optJSONObject("library");
                renderLibraryList();
            }
        });
    }

    private void renderLibraryList() {
        libraryListContainer.removeAllViews();
        if (libMode == 0) {
            JSONArray tracks = lastLibrary.optJSONArray("tracks");
            if (tracks == null || tracks.length() == 0) {
                libraryListContainer.addView(makeSectionHeading("No tracks found"));
                return;
            }
            for (int i = 0; i < tracks.length(); i++) {
                JSONObject t = tracks.optJSONObject(i);
                if (t == null) continue;
                String path = t.optString("path", "");
                LinearLayout row = makeRow();
                row.setBackground(roundedBg(0xFF131826, 0x18FFFFFF, 10));
                row.setPadding(dp(10), dp(8), dp(8), dp(8));

                LinearLayout info = new LinearLayout(this);
                info.setOrientation(LinearLayout.VERTICAL);
                TextView tv1 = new TextView(this);
                tv1.setText(t.optString("title", "Track"));
                tv1.setTextColor(0xFFF8FAFC);
                tv1.setTextSize(13f);
                tv1.setMaxLines(1);
                tv1.setEllipsize(TextUtils.TruncateAt.END);
                TextView tv2 = new TextView(this);
                tv2.setText(t.optString("artist", "") + " · " + ThemeColors.formatTime(t.optDouble("duration", 0)));
                tv2.setTextColor(0xFF94A3B8);
                tv2.setTextSize(11f);
                tv2.setMaxLines(1);
                info.addView(tv1);
                info.addView(tv2);
                info.setOnClickListener(v -> client.sendWith("play_path", o -> { try { o.put("path", path); } catch (Exception ignored) {} }, null));
                row.addView(info, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));

                row.addView(makeBtn("+Q", false, v -> client.sendWith("add_to_queue", o -> { try { o.put("path", path); } catch (Exception ignored) {} }, null)));
                libraryListContainer.addView(row);
            }
        } else if (libMode == 1) {
            JSONArray folders = lastLibrary.optJSONArray("folders");
            if (folders == null || folders.length() == 0) {
                libraryListContainer.addView(makeSectionHeading("No folders found"));
                return;
            }
            for (int i = 0; i < folders.length(); i++) {
                JSONObject f = folders.optJSONObject(i);
                if (f == null) continue;
                String path = f.optString("path", "");
                LinearLayout row = makeRow();
                row.setBackground(roundedBg(0xFF131826, 0x18FFFFFF, 10));
                row.setPadding(dp(10), dp(8), dp(8), dp(8));
                TextView tv = new TextView(this);
                tv.setText("📁 " + f.optString("name", "Folder") + " (" + f.optInt("track_count", 0) + ")");
                tv.setTextColor(0xFFF8FAFC);
                tv.setTextSize(13f);
                row.addView(tv, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
                row.addView(makeBtn("▶ Play", true, v -> client.sendWith("play_folder", o -> { try { o.put("path", path); o.put("shuffle", false); } catch (Exception ignored) {} }, null)));
                row.addView(makeBtn("🔀", false, v -> client.sendWith("play_folder", o -> { try { o.put("path", path); o.put("shuffle", true); } catch (Exception ignored) {} }, null)));
                libraryListContainer.addView(row);
            }
        } else {
            libraryListContainer.addView(makeBtn("Clear Queue", false, v -> client.sendCmd("clear_queue")));
            JSONArray q = lastStatus.optJSONArray("queue_items");
            JSONArray pl = lastStatus.optJSONArray("playlist_items");
            addQueueItems(q, true);
            addQueueItems(pl, false);
        }
    }

    private void addQueueItems(JSONArray arr, boolean queued) {
        if (arr == null) return;
        for (int i = 0; i < arr.length(); i++) {
            JSONObject item = arr.optJSONObject(i);
            if (item == null) continue;
            int idx = item.optInt("playlist_index", i);
            LinearLayout row = makeRow();
            row.setBackground(roundedBg(0xFF131826, 0x18FFFFFF, 10));
            row.setPadding(dp(10), dp(8), dp(8), dp(8));
            TextView tv = new TextView(this);
            String prefix = queued ? "[Q] " : (item.optBoolean("is_current", false) ? "▶ " : "");
            tv.setText(prefix + item.optString("title", ""));
            tv.setTextColor(item.optBoolean("is_current", false) ? accentColor : 0xFFF8FAFC);
            tv.setTextSize(13f);
            row.addView(tv, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
            row.addView(makeBtn("▶", false, v -> client.sendWith(queued ? "play_queue_item" : "play_upcoming",
                    o -> { try { o.put("index", idx); } catch (Exception ignored) {} }, null)));
            libraryListContainer.addView(row);
        }
    }

    private void refreshAndroidStatus() {
        client.sendWith("get_android_status", null, resp -> {
            if (resp != null && resp.has("android") && !resp.isNull("android")) {
                lastAndroidStatus = resp.optJSONObject("android");
                renderAudioBtStatus();
            }
        });
    }

    private void renderAudioBtStatus() {
        String src = lastAndroidStatus.optString("active_source", "pc");
        boolean rev = lastAndroidStatus.optBoolean("reverse_bt_active", false);
        audioSourceSummary.setText("Active Source: " + ("android".equals(src) ? "Android (Reverse BT)" : "PC (tntron)")
                + (rev ? " · A2DP Stream Active" : ""));

        JSONArray players = lastAndroidStatus.optJSONArray("media_players");
        if (players != null && players.length() > 0) {
            JSONObject p = players.optJSONObject(0);
            String title = p.optString("title", "");
            avrcpTitleView.setText(title.isEmpty() ? p.optString("device_name", "Bluetooth Player") : title);
            avrcpSubView.setText(p.optString("artist", "") + " · " + p.optString("status", "idle").toUpperCase());
        } else {
            avrcpTitleView.setText("No active Bluetooth media player");
            avrcpSubView.setText("Connect phone via Bluetooth & play audio to control via AVRCP");
        }

        sinksContainer.removeAllViews();
        JSONArray sinks = lastAndroidStatus.optJSONArray("sinks");
        if (sinks != null && sinks.length() > 0) {
            for (int i = 0; i < sinks.length(); i++) {
                JSONObject s = sinks.optJSONObject(i);
                if (s == null) continue;
                String name = s.optString("name", "");
                String desc = s.optString("description", name);
                boolean def = s.optBoolean("is_default", false);
                Button b = makeBtn((def ? "✓ " : "") + (s.optBoolean("is_bluetooth", false) ? "[BT] " : "") + desc,
                        def, v -> client.sendWith("set_audio_sink", o -> { try { o.put("sink_name", name); } catch (Exception ignored) {} },
                                r -> refreshAndroidStatus()));
                LinearLayout.LayoutParams lp = new LinearLayout.LayoutParams(ViewGroup.LayoutParams.MATCH_PARENT, ViewGroup.LayoutParams.WRAP_CONTENT);
                lp.bottomMargin = dp(6);
                sinksContainer.addView(b, lp);
            }
        } else {
            sinksContainer.addView(makeSectionHeading("No PipeWire sinks reported"));
        }

        btDevicesContainer.removeAllViews();
        JSONArray devs = lastAndroidStatus.optJSONArray("devices");
        if (devs != null && devs.length() > 0) {
            for (int i = 0; i < devs.length(); i++) {
                JSONObject d = devs.optJSONObject(i);
                if (d == null) continue;
                String path = d.optString("object_path", "");
                String name = d.optString("alias", d.optString("name", d.optString("address", "Device")));
                boolean conn = d.optBoolean("connected", false);
                LinearLayout row = makeRow();
                TextView tv = new TextView(this);
                tv.setText((conn ? "● " : "○ ") + name);
                tv.setTextColor(conn ? accentColor : 0xFFCBD5E1);
                tv.setTextSize(13f);
                row.addView(tv, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
                row.addView(makeBtn(conn ? "Disconnect" : "Connect", false, v ->
                        client.sendWith(conn ? "bt_disconnect" : "bt_connect",
                                o -> { try { o.put("device_path", path); } catch (Exception ignored) {} },
                                r -> refreshAndroidStatus())));
                row.addView(makeBtn("Stream -> PC", true, v ->
                        client.sendWith("bt_reverse_audio",
                                o -> { try { o.put("device_path", path); } catch (Exception ignored) {} },
                                r -> refreshAndroidStatus())));
                btDevicesContainer.addView(row);
            }
        } else {
            btDevicesContainer.addView(makeSectionHeading("No paired Bluetooth devices"));
        }
    }

    private void refreshThemes() {
        client.sendWith("get_themes", null, resp -> {
            if (resp != null && resp.has("themes") && !resp.isNull("themes")) {
                lastThemes = resp.optJSONArray("themes");
                themesContainer.removeAllViews();
                if (lastThemes == null) return;
                LinearLayout curRow = null;
                for (int i = 0; i < lastThemes.length(); i++) {
                    if (i % 3 == 0) {
                        curRow = makeRow();
                        themesContainer.addView(curRow);
                    }
                    JSONObject th = lastThemes.optJSONObject(i);
                    if (th == null) continue;
                    String slug = th.optString("slug", "openify");
                    String lbl = th.optString("label", slug);
                    boolean active = slug.equalsIgnoreCase(lastStatus.optString("active_theme", ""));
                    addWeightedBtn(curRow, makeBtn((active ? "✓ " : "") + lbl, active, v ->
                            client.sendWith("set_theme", o -> { try { o.put("slug", slug); } catch (Exception ignored) {} }, null)), 1f);
                }
            }
        });
    }

    private void refreshExtensions() {
        client.sendWith("get_extensions", null, resp -> {
            if (resp != null && resp.has("extensions") && !resp.isNull("extensions")) {
                lastExtensions = resp.optJSONArray("extensions");
                extensionsContainer.removeAllViews();
                if (lastExtensions == null) return;
                for (int i = 0; i < lastExtensions.length(); i++) {
                    JSONObject ext = lastExtensions.optJSONObject(i);
                    if (ext == null) continue;
                    String id = ext.optString("id", "");
                    String name = ext.optString("name", id);
                    boolean en = ext.optBoolean("enabled", false);
                    LinearLayout row = makeRow();
                    TextView tv = new TextView(this);
                    tv.setText(name);
                    tv.setTextColor(en ? accentColor : 0xFFCBD5E1);
                    tv.setTextSize(13f);
                    row.addView(tv, new LinearLayout.LayoutParams(0, ViewGroup.LayoutParams.WRAP_CONTENT, 1f));
                    row.addView(makeBtn(en ? "ON" : "OFF", en, v ->
                            client.sendWith("toggle_extension", o -> { try { o.put("id", id); } catch (Exception ignored) {} },
                                    r -> refreshExtensions())));
                    extensionsContainer.addView(row);
                }
            }
        });
    }
}
