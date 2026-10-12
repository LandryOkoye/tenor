Make a dynamic 2-minute motion graphics demo film for Tenor (http://localhost:5173/) to submit to a
hackathon. It should have the energy of a motion designer's showreel, but its first job is to prove to
judges that the product is real and works. Keep it clean and premium.

**Register: 70% product walkthrough, 30% launch film, blended — not alternating.**
See <blend_doctrine> below; it governs every other section.

<inputs>
Product: Tenor (http://localhost:5173/) — a prediction market app that makes trading on real-world
  events feel like investing, not gambling. Users trade YES/NO positions on real events (sports,
  politics, crypto, economics, entertainment). What is different:
  1. Baskets: buy a curated bundle of several markets around one theme in a single signature. Unlike
     a parlay (all legs must win), a basket pays out per leg, so being partly right still pays.
  2. Resolution Center: every market's resolution is shown in the open — rules and source, an evidence
     timeline, status, and a track record for each market creator.
  Tenor is non-custodial: users keep their funds and positions in their own Solana wallet.
  Built with a Rust backend (axum) and a React frontend, on Solana, settled in USDC, using Panta
  (panta.market) for markets, liquidity and settlement.
Tagline: pull it off the site (hero H1). If the hero copy is longer than ~8 words, use the shortest
  complete clause and tell me what you picked. If the site has no hero tagline, propose three options in
  the spirit of "Trade your view of the world" and wait for me to pick.
Audience: hackathon judges (technical and product), then crypto-curious traders on X.
Call to action: pull the exact label and URL from the site's primary CTA button. If there is no live
  domain yet, use: "Try Tenor · [DEMO_URL]" plus the GitHub repo URL, and flag both so I can correct
  them.
Persistent stamp: "BUILT ON SOLANA · POWERED BY PANTA". Tenor is NOT publicly launched, so never write
  "NOW LIVE", "launched", or "available now" anywhere. "Powered by Panta" must use that exact wording
  (Panta's terms require it). See <blend_doctrine> for how the stamp is used.
Demo-data label: every walkthrough shot shows real Tenor UI running on the app's recorded-fixture
  mock data (no real funds were used). Panta's terms forbid presenting simulated data as live, so a
  small "DEMO DATA" label sits in a fixed corner (opposite the stamp) for the whole walkthrough,
  0:16–1:40. It sets with the first UI shot and never animates until it leaves at the proof.
Format: 1920x1080 (16:9) first — this is the hackathon submission. Then 1:1 and 9:16 cut-downs of
  45–60s from the same timeline, for posting on X.
Music: original track synthesized in code, 120 BPM. 2:00 at 120 BPM = 240 beats = 60 bars of 4/4.
  Structure it as a launch cue, not a loop: 2 bars of restraint, a drop on the product reveal, then a
  steady, relatively flat bed under the walkthrough so the UI can be read, with a small lift at each
  section change (markets → baskets → portfolio → resolution → under the hood), a bigger lift into the
  proof, and a clean resolve on the lockup. The track peaks at the ends, not in the middle.
Voiceover: my own cloned voice via Fish Audio, voice model ID 252198f671e7439b99feff1aefd27021
  (https://fish.audio/m/252198f671e7439b99feff1aefd27021). See <voiceover> for API setup.
</inputs>

<honesty_rules>
This is a hackathon demo. Judges will check claims.
- Only show features that actually exist and work in the app. If a screen listed in <assets> does not
  exist yet or is broken, STOP and tell me. Do not mock it up, redraw it, or fake it.
- Never invent user counts, volume, revenue, partnerships, or time-saved figures.
- All data on screen comes from ONE source: the app's recorded-fixture mock mode (`PANTA_MOCK=true`,
  see <assets>). It is real Tenor UI on demo data, labelled "DEMO DATA" throughout the walkthrough.
  Never imply it is live market data or that real money moved.
- Show only what the app actually displays. In particular, with the demo data:
  - no third-party UI exists to show: there is NO wallet popup and NO block-explorer transaction
    (nothing was broadcast to Solana). Do not draw, borrow or imply either;
  - a basket is ONE transaction: all legs fill at the same instant or none do — never show legs
    filling one after another;
  - the evidence timeline has no explorer links; the demo markets do not change state on their own;
    no basket leg resolves during the film and there is nothing to claim;
  - price history may be flat or empty; Panta returns no price-impact figure (the UI shows "—").
- Prices, markets and amounts must be consistent across every shot.
- Roadmap items (social trading via WhatsApp/Telegram, tokenized baskets, naira on-ramp) appear ONLY in
  the "What's next" beat and are clearly labelled as next, never shown as working UI.
</honesty_rules>

<blend_doctrine>
The two registers must coexist in every beat, not take turns. Concretely:

**Time split (the 70/30).** 84 of the 120 seconds are real UI with a real action happening
(walkthrough). 36 seconds are the cold open, hook, reveal, proof, what's next and lockup (launch). The
walkthrough is the body of the film, not an interlude between title cards. Judges need to see the
product working.

**The walkthrough supplies the content; the launch supplies the cutting and the scoring.** Every
feature beat shows a genuine, legible product moment, but framed, timed and scored like a launch film:
decisive camera moves, hard seats, one accent, no filler motion.

**Legibility is the walkthrough's non-negotiable.** Every walkthrough beat contains at least one
*static hold* of at least 1s where the camera stops, nothing animates, and the real UI is readable at
100%: the market question and price, the basket legs and fee breakdown, the portfolio values, the
resolution timeline. A judge must be able to pause on that frame and understand the product. Animate
in, hold still, animate out.

**The launch register persists through the middle, quietly.** The "BUILT ON SOLANA · POWERED BY PANTA"
stamp sets small in a fixed corner at the product reveal and stays there for the rest of the film,
locked in place while everything else moves. Never animate it after it sets, until the lockup. The
"DEMO DATA" label behaves the same way in the opposite corner for the walkthrough.

**Voiceover carries the shift.** The ends are declarative launch copy. The middle lines are plain
explanatory product narration — what the product is doing, in order. No marketing adjectives in the
feature lines; the walkthrough earns its credibility by sounding like someone showing you how it works.

**Test for the blend:** if you cut the first 16 and last 20 seconds, what remains should stand alone as
a credible product demo. If you cut the middle 84, what remains should stand alone as a teaser. If
either half fails that test, the blend is wrong.
</blend_doctrine>

<creative_spine>
"Tenor" means two things: in finance, the time until something matures; in music, a voice that holds
a clear note. Tenor turns quick bets into views you hold until they mature.

Use this as the sound and motion signature, not as a visual pun:
- **Sound:** one clean, sustained tonal note (the "tenor tone"), the same pitch every time. It opens
  the film, sounds softly when the basket purchase completes and when the resolved outcome appears in
  the Resolution Center, and closes the film on the wordmark.
- **Motion:** a single thin horizontal accent line, the "maturity line", running left to right. It is
  the cold-open hairline, it becomes the timeline in the Resolution Center beat, and it settles under
  the wordmark at the end. UI elements seat into place with a hard stop, never a bounce.
- **Never** show casino or gambling imagery: no dice, chips, cards, roulette, slot machines, coins
  flipping, money rain, or "bet" language on screen. The whole point is that this is not gambling.
- Do not draw a musical note, a microphone, a clock or an hourglass.
</creative_spine>

<assets>
- Visit the site with Playwright. Capture real screenshots (desktop, retina 2x), the real logo (SVG if
  available), exact brand colors (hex) and fonts. Pick one accent color from the brand.
- **Before screenshotting, put the app in a known state (one data source, demo data):**
  - Backend: from the repo root run `RUN_JOBS=true ./scripts/dev-mock.sh` (Postgres + the Rust API
    with mock Panta, the background jobs, and three seeded baskets) on http://localhost:8080.
  - Frontend: in `frontend/`, run `pnpm dev` with `VITE_API_BASE_URL=http://localhost:8080` and
    `VITE_MOCK` UNSET (the frontend's own built-in fakes would show different data and break
    continuity).
  - To show a basket in the portfolio, buy the demo basket once through the UI with the dev wallet
    first (mock mode signs nothing real); tell me if the UI needs a wallet you cannot connect.
  - Wait for network idle. Inject `prefers-reduced-motion` and hide scrollbars so every capture is a
    settled frame.
- **Keep continuity.** The same basket name, the same legs (market questions, sides, prices), the same
  wallet address and the same dates must appear in every shot. Mismatched names or prices between
  beats is the most noticeable error in a demo film.
- **Capture at walkthrough quality.** Every capture must be sharp at 2x, with no truncated strings, no
  lorem, no placeholder avatars, no error toasts, and plausible dates relative to today. An empty
  price chart is acceptable only if it shows the app's own "history since we started tracking" note.
- Capture these screens, each at desktop 2x and at mobile viewport (390x844 @2x):
  1. Markets list with category tabs, search and real prices (shown as ¢ and %)
  2. Market detail: question, rules, close time, price (and chart if it has history), trade tape, buy
     panel
  3. Baskets list with basket cards (title, legs count, index price, closes in)
  4. Basket detail: thesis, leg table (market, side, weight, live price) and the buy panel with the
     per-leg breakdown and the fee lines (Tenor fee, Panta fee)
  5. The pre-signature summary ("You are buying N positions for X USDC…")
  6. The purchase result with every leg filled (all at once — one transaction)
  7. Portfolio with the basket: its legs, value, cost basis, P&L, legs resolved X of N
  8. Resolution Center hub (Resolving soon / Recently resolved / Delayed)
  9. A resolved market's resolution page: rules and source, evidence timeline, outcome, creator
     scorecard, "Tenor analysis" labels
  10. A creator scorecard page
  11. The "Powered by Panta" footer
- Also capture hover and active states for any element the cursor will touch.
- There is no wallet popup and no explorer transaction to capture (see <honesty_rules>); do not ask
  for or use either.
- If any screen needs a login or wallet connection you can't do, tell me which one and I'll provide a
  screenshot.
- Save everything to ./assets and list what you found before animating anything.
- Never redraw the product UI from imagination. Crop, mask, zoom and animate the real screenshots.
</assets>

<story>
Timings are targets on the 240-beat grid (0.5s per beat). Snap the real cuts to the VO. The walkthrough
beats total 84s and must not be compressed to make room elsewhere — if something has to give, shorten
the cold open, what's next and the lockup.

0. **Cold open (0:00–0:03 · 6 beats).** Warm canvas, empty. The accent maturity line draws across. The
   tenor tone sounds once on the downbeat. No logo yet.

1. **Hook (0:03–0:10 · 14 beats).** Huge kinetic type, two lines set on the beat:
   **"A bet is a guess."** — then — **"A view is an investment."**
   Nothing else on screen. No VO here.

2. **The product appears (0:10–0:16 · 12 beats).** The drop. The real tagline sets, and the real UI
   assembles piece by piece — chrome first, then cards, then prices — each piece seating with a hard
   stop. Do not cross-dissolve a screenshot in. **The "BUILT ON SOLANA · POWERED BY PANTA" stamp sets
   small in a fixed corner here and remains for the rest of the film.**

3. **The walkthrough — real UI, a cursor doing real actions. Body of the film (84s, 70%).** The
   "DEMO DATA" label sets in the opposite corner on the first walkthrough frame and stays still.

   a. **Markets (0:16–0:28 · 24 beats).** The cursor moves across category tabs on the markets list
      and opens one market. **HOLD 1s: question, YES price in ¢ and %, close time readable.** Then the
      trade tape and buy panel. Establishes the basics every prediction market user expects. (Show
      the price chart only as captured — never animate a chart drawing in.)

   b. **Baskets (0:28–0:52 · 48 beats). The hero beat — give it the most time.** The cursor opens the
      Baskets list and picks the demo basket. **HOLD 1.25s on the leg table: every market, side,
      weight and price readable.** The cursor enters an amount; the per-leg breakdown fills in line by
      line. **HOLD 1.25s on the breakdown with the Tenor fee and Panta fee readable.** The cursor taps
      Buy; the pre-signature summary appears. **HOLD 1s on the summary** ("You are buying N positions
      for X USDC…"). Cut straight to the result — no wallet UI: every leg flips to filled at the same
      instant, seating with one hard stop, and the tenor tone sounds once. **HOLD 1s on "all legs
      filled".** The point of this beat: several markets, one signature, all-or-nothing, and a clear
      statement of exactly what the user signed.

   c. **Portfolio (0:52–1:08 · 32 beats).** The basket appears in the portfolio with its legs.
      **HOLD 1s: legs resolved X of N, value, cost basis, P&L readable.** The camera moves once to the
      leg list: each leg is its own position with its own value. Nothing resolves here; the VO
      explains that when the markets resolve, each winning leg pays on its own — being partly right
      still pays.

   d. **Resolution Center (1:08–1:32 · 48 beats).** The maturity line returns and becomes the
      evidence timeline. The Resolution hub appears (Resolving soon / Recently resolved / Delayed). The
      cursor opens a resolved market. **HOLD 1.25s on the timeline and the outcome readable** (the
      tenor tone sounds softly as the outcome seats). The camera moves once to the creator scorecard.
      **HOLD 1s on the scorecard with the "Tenor analysis" label readable.** End the beat on a still,
      cursor-free hold of the hub — nothing animates, the open record speaks for itself.

   e. **Under the hood (1:32–1:40 · 16 beats).** A clean, typographic architecture strip built in the
      film's own style: React → Rust (axum) → Panta API → Solana, with "non-custodial: your wallet,
      your keys" beneath it. **HOLD 1s.** For technical judges.

4. **Proof (1:40–1:48 · 16 beats).** One large statement composited over the real pre-signature
   summary screenshot (not an empty canvas), using only true facts from the demo, e.g.
   **"3 markets. 1 signature."** (use the demo basket's real leg count). No VO on the number itself.

5. **What's next (1:48–1:54 · 12 beats).** Three small typographic chips, clearly labelled "Next":
   **Trade on WhatsApp & Telegram · Tokenized baskets · Naira on-ramp.** No fake UI.

6. **Lockup (1:54–2:00 · 12 beats).** Wordmark with the maturity line settling beneath it, the stamp
   moving from its corner to full size beneath the wordmark, and the CTA. Final tenor tone on the
   wordmark. Hold clean.

Running split: walkthrough 0:16–1:40 = 84s (70%). Launch 0:00–0:16 + 1:40–2:00 = 36s (30%).
</story>

<direction>
- Showreel energy, but read <blend_doctrine> first: energy means decisive, not constant. The static
  holds are deliberate and must survive the edit.
- Warm, clean canvas, one accent color from the brand. Springs with at most a tiny overshoot.
- Text inside a moving container enters after the move starts and leaves before the next one.
- **Camera discipline:** at most one camera move per walkthrough sub-beat. Move, then stop, then hold.
- **Launch register:** the cold open sits in near-silence, resolves hard rather than fading, and the
  persistent stamp stays absolutely still while everything around it moves.
- Banned: bouncy easing, glows, gradients on UI chrome, particle bursts, confetti, stock-looking
  effects, countdowns, "launching soon" furniture, 3D device tilts, speed-ramped UI scrolling,
  candlestick-chart clichés, chart-drawing animations, any gambling imagery, and any shot where text
  moves while it is meant to be read.
</direction>

<voiceover>
Generated with my cloned voice through the Fish Audio API.

**API setup (required — the agent cannot use the voice without this):**
- The Fish Audio API needs an API key. I will create one in my Fish Audio account (API keys section of
  the dashboard) and put it in a local `.env` file as `FISH_AUDIO_API_KEY`. Read it from the
  environment only. Never print it, log it, paste it into code, or commit it. Make sure `.env` is in
  `.gitignore`.
- API usage is billed against my Fish Audio API balance. Check that the key works with one short test
  line before generating the full script, and tell me if the request fails with an auth or balance
  error.
- Call the text-to-speech endpoint with the key as a Bearer token and the voice model ID
  `252198f671e7439b99feff1aefd27021` as the reference/voice ID. Check Fish Audio's current API docs for
  the exact endpoint, headers (including any model/version header) and body fields before writing the
  request; do not guess them.
- Pronunciation: "Tenor" = TEN-er; "Panta" = PAN-tah; "Solana" = so-LAH-na. If any is misread,
  regenerate that line, adjusting spelling in the request text if needed.

**Script:**
- Write a short, confident voiceover, one line per beat, about 10–12 spoken lines total: one on the
  reveal, one or two on each walkthrough beat, one on under the hood, one on what's next, one on the
  lockup.
- **Register split per <blend_doctrine>:** reveal, what's next and CTA lines are declarative launch
  copy. Walkthrough lines are plain explanatory narration — subject, verb, object, present tense,
  naming what is happening on screen without adjectives. "Tenor buys all three markets in one
  transaction" beats "seamless, powerful basket trading."
- The VO must explain the basket difference plainly once: unlike a parlay, a basket pays out per leg.
- The VO must not claim anything the demo data does not show: no "live", no real money moving, no
  wallet popup, no explorer.
- Total spoken time about 75–85s, so the music and holds have room. Walkthrough lines carry most of it.
- Do NOT read the kinetic-type hook or the proof number. The VO complements on-screen text, never
  repeats it. Cold open, hook and proof have no VO.
- **Show me the script and STOP.** I'll approve or rewrite it before you spend credits on audio.

**When I say "go":**
1. Generate one file per line into ./assets/vo/vo_01.wav, vo_02.wav, etc. Keep the raw returns; don't
   overwrite them.
2. Transcribe each file with Whisper (word-level timestamps) and confirm it matches the script. If a
   line is misread or mispronounced ("Tenor" and "Panta" especially), regenerate that line only and
   tell me.
3. Trim silence, remove noise lightly, normalize loudness (about -16 LUFS), add gentle compression.
4. Build the timeline around the real delivery: each cut snaps to the nearest musical beat after the
   line ends. Stretch the visuals, never the voice. **Protect the static holds:** if a line runs long,
   extend the beat rather than eating into its hold. If the film runs past 2:00, shorten the hook,
   what's next and lockup first; tell me if it still runs over.
5. Duck the music 8–10 dB under the voice (sidechain style). The tenor tone, UI ticks and whooshes stay
   on the beat and are NOT ducked.
</voiceover>

<build>
1. One HTML file, one canvas, driven by window.seek(t). No CSS transitions, no timers, no carried state.
2. Closed-form springs. A value with many targets is the sum of one spring per change.
3. Beat grid from the synthesized track (numpy/librosa). Start on a downbeat. UI sounds on measured
   peaks.
4. Render in headless Chrome at 60 fps, 4 subframes per frame blended for motion blur.
   **Disable subframe blending during the static holds** — it risks softening text meant to be read.
5. Mux video, music, SFX and voiceover with ffmpeg into MP4 (H.264, AAC). Keep the 16:9 file under the
   hackathon's upload size limit; ask me for the limit if you don't know it.
6. Also export burned-in captions (SRT and a captioned MP4), since many judges watch muted.
</build>

<gotchas>
- Never use will-change on anything the camera scales (it blurs text).
- The last frame must match the first so the video can loop: warm canvas, the accent maturity line,
  the wordmark at rest.
- **Verify every static hold at 100% zoom on the rendered frame, not in the browser preview.** If the
  text in a hold isn't crisply readable in the actual MP4, the walkthrough has failed.
- Prices and values must be consistent across beats (the basket's legs must show the same prices in
  the basket detail, the breakdown and the portfolio).
- The "DEMO DATA" label must be readable at 1080p and present in every walkthrough frame, including
  the cut-downs.
- 1:1 and 9:16 cut-downs (45–60s: hook, reveal, baskets, resolution, lockup) must reframe the layout,
  not just crop it. Use the mobile-viewport captures for UI beats, keep all text in safe areas, and
  lengthen each static hold by ~0.25s.
- All screenshots must come from one dataset. Check names, prices and dates match before animating.
</gotchas>

<start>
Phase 1, then stop and wait for me:
1. Confirm FISH_AUDIO_API_KEY is set (without printing it) and run one short test line.
2. Capture the assets and list what you found, including the tagline and CTA you pulled, the accent
   hex you chose, any screen in <assets> that doesn't exist or looks broken, and anything behind a
   login or wallet connection you could not capture.
3. Show the story as a state list on the beat grid (240 beats, 60 bars), with every static hold marked
   and the walkthrough-versus-launch split stated so I can confirm it lands at 70/30.
4. Show the voiceover script, one line per beat, labelled launch or walkthrough, with estimated
   duration per line.
After I approve the script and say "go":
5. Generate the VO, then show a contact sheet with one frame per beat, plus a separate full-size frame
   for every static hold so I can check legibility before the full render.
6. After I approve it, render the 16:9 film, then the 1:1 and 9:16 cut-downs.
</start>
