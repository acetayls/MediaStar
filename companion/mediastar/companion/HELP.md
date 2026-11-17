## MediaStar (JingleStar, CueStar)

One connection drives one application. Add a second connection for the other one, or for another computer.

### Setup in the application

1. Open the remote controls (Preferences) and **enable OSC**.
2. **Enable OSCQuery**: Companion reads the state from it (names, colours, time left, cue in standby). Without it, the buttons still send their commands but show no state.
3. CueStar on another computer: also allow **network access** (CueStar listens on this computer only by default).

### Connection settings

- **Application**: JingleStar or CueStar.
- **Computer**: IP address of the computer running the application (`127.0.0.1` for this one).
- **OSC port / OSCQuery port**: 0 uses the defaults (JingleStar 8000 / 8001, CueStar 9000 / 9001).
- **OSC prefix**: empty uses `/jinglestar` or `/cuestar`.
- **State refresh**: how often the state is read (250 ms by default).

### JingleStar

- Actions: stop all, GO; per module: stop, volume (set or adjust), mode; jingle play / stop / toggle and volume; playlist transport, track number, pause, shuffle and loop; cue list GO, next / previous cue, cue number in standby, pause.
- Variables (module `m` 1-4, jingle `j` 1-12): `m1_name`, `m1_j3_name`, `m1_j3_remaining`, `m1_pl_title`, `m1_pl_remaining`, `m1_cue_name`, `m1_cue_playing_name`, `m1_cue_remaining`…
- Feedbacks: jingle colour (bright when playing), jingle playing, playlist playing / paused / shuffle / loop, cue playing / paused, module mode.
- Presets: a grid of 12 jingles per module, playlist transport, GO button with the cue in standby.

The jingle, playlist and cue list values only exist while the module is in that mode in JingleStar.

### CueStar

- Actions: black, cue list transport (GO, play, pause, stop, next, previous), cue in standby, go to cue, loop, transition; slideshow transport, media number and settings.
- Variables: `media`, `time`, `remaining`, `cue_standby`, `cue_name`, `cue_running`, `cue_count`, `slide_index`, `slide_count`.
- Feedbacks: paused, media on screen, black, loop, cue in standby is number.

### Stream Deck

Companion drives the Stream Deck: drag the presets onto the buttons.

---

## En français

Une connexion pilote une appli (JingleStar ou CueStar). Dans l'appli : Préférences, télécommandes, activer **OSC** et **OSCQuery** (l'état des boutons vient d'OSCQuery). CueStar sur un autre PC : autoriser aussi l'**accès réseau**. Ports à 0 et préfixe vide : valeurs par défaut de l'appli. Les presets se glissent sur les boutons du Stream Deck.
