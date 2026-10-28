# Broadcast guide

RaceDirector v0.1.19 adds battle framing and coverage graphics to the existing Airframe controls. This first broadcast update passed 71 automated tests. The new features still need in-game validation across race modes and tracks.

## Set up a race

1. Enter a race and press **Insert**.
2. Under **Tower > Trainers**, optionally enable trainer names and enter them by gate. Review these labels whenever the field changes.
3. Open **On air** and choose **Show coverage graphics**. This enables the timing tower and all four coverage graphics. Individual toggles let you choose a smaller layout.
4. Enter an event/round label, course name and race distance under **Event details**. These are operator-supplied values, saved between races. A distance of **0** uses game-supplied course length when available, falling back to distance covered.
5. Under **Camera**, enable camera control with **F6**, choose **Follow** and select the runner to feature.
6. Press **F8** for clean capture. The cursor and settings panel disappear while enabled broadcast graphics remain visible. F8 or Escape releases capture.

Graphics use the side opposite the timing tower and scale to fit the available area. The tower's Left/Right setting also determines the graphics side when the tower is hidden. All overlay windows ignore mouse input.

## Battle camera

Press **F9** while camera control and Follow mode are active, or enable **Battle camera** under Camera > Mode. The camera frames the selected runner and one rival together. Existing target transitions, camera smoothing and ground protection remain active. Framing can widen immediately when needed to avoid cropping either runner.

**Auto / nearest rival** considers the runners immediately ahead and behind in race order. The default minimum hold is **4 seconds**, and the default maximum track-distance gap is **30 metres**. A different contender must be meaningfully closer before auto pairing switches after the hold. The pair stays together when its runners exchange positions.

The partner menu can pin a gate manually. A missing or out-of-range manual partner does not silently select another runner. Normal follow continues when no eligible pair is available. Finished runners are excluded from pairing. Manual partners are saved by gate, so review them for each new field.

Under **Battle selection**, adjust the minimum hold and maximum gap. Switch targets with **[ / ]** or the runner menu. Battle mode is suspended in Freecam; camera movement there stays under your control.

## Coverage graphics

| Graphic | Display and data |
| --- | --- |
| Runner spotlight | Gate badge, runner name, optional trainer, position and speed for the followed runner. It requires camera control in Follow mode. |
| Battle card | Both runners, the position being contested, distance gap and relative closing speed. It replaces the spotlight while an eligible battle is active. |
| Race progress | Event/course labels and the leader's distance progress. Remaining distance uses the manual override or game-supplied course length. With neither, it shows distance covered. |
| Finish and results | Confirmed arrivals, a winner card once the first-place runner has finished, and final results once the field has finished. |

The spotlight uses a gate badge; character portraits are not loaded. Trainer names come from **Tower > Trainers**. v0.1.20 adds automatic game-supplied names with nonblank manual overrides taking priority. Missing telemetry displays as unavailable rather than an invented value. No final-straight or race-phase boundary is inferred from the entered race distance.

The battle card's closing speed is the trailing runner's speed minus the leading runner's speed. Positive values indicate closing; negative values indicate the gap opening. These values use current race telemetry, whose last speed can remain visible while paused. Timing-tower seconds remain estimates from distance divided by the trailing runner's speed, not measured crossing-time intervals.

When confirmed finish results are displayed, they take priority over progress and runner cards so the full field stays readable at smaller resolutions.

Replay data can contain final positions before the race ends. RaceDirector only reads and presents final order for runners the game reports as finished. It does not predict the winner.

## Timing tower

Under **Tower > Appearance**, use **Animate position changes** to let rows glide into their new order. **Show places gained / lost** adds a short-lived positive or negative indicator after an order change. Both preferences are saved and default to enabled. Disable animation for a static tower.

The followed runner is highlighted, and a stripe marks the battle partner. Trainer labels remain attached to their gate when rows move. New fields, replay rewinds and race exits clear presentation history, avoiding false overtake indicators. Live rank and gap values continue to use the latest telemetry while row positions animate.

## Restore and persistence

**F10** restores the game's camera and visual state, hides coverage graphics and disables battle mode and automatic directing. These disabled states save automatically. Event details, trainer labels, appearance preferences and other configuration values remain saved for reuse.

Broadcast cards start disabled when upgrading from an older version. The v0.1.22 focus markers default on while Follow camera is active. **Show coverage graphics** enables them together; **Hide coverage graphics** hides the tower and all coverage cards without changing the camera. The **Help > Save settings now** button writes immediately.

This update does not include a 2D track map, replacement skill feed, separate OBS browser overlays or replay capture. Those require additional integration and validation.

## Automatic director (v0.1.23)

Enable **Camera > Automatic director > Automatic directing**. This now owns its prerequisites: camera control becomes enabled, Freecam changes to Follow and finish coverage is enabled. **Include battles** controls automatic pairing independently of the manual Battle toggle. It frames a nearby rival around each touring runner and the selected runner afterward; it does not override a saved manual partner preference.

The panel displays its current action, missing inputs and the course-length source. **Race length override (0 = Auto)** uses the game value when available; an explicit positive override takes priority. Game data remains local to this race. The current replay's course-length and world-scale getters were verified in the running game.

Under **Continuous movement**, **Gentle camera movement** adds slow orbit, dolly, height and lens changes around your chosen framing. It starts immediately, even before the tour threshold, and runs independently of target changes. The default movement amount is 1.0 with a 24-second base cycle. It also runs while paused; disable it when a still pause frame is wanted. Freecam and manual Follow are unaffected. A fixed finish shot retains its world position.

Under **Runner tour**, choose its start fraction and seconds per runner (defaults 25% and 6 seconds), or use **Start / restart runner tour now**. When no course length is available, the tour starts at 200 m. It begins at the selected runner and chooses the nearest unvisited runner by race distance, retaining one continuous camera rig rather than alternating sides or lenses for every runner. Completion returns to the selected runner and preferred framing while gentle movement continues.

Tour holds have a 5-second minimum; saved shorter values are raised to that minimum. They use viewing time while race telemetry advances, so replay fast-forward cannot accelerate the cuts. Pausing stops runner changes without advancing their hold timer. Automatic target transitions take at least 1.8 seconds and framing eases more slowly. Swapping focus and rival within the same pair retains the same camera identity and orientation. During automatic travel, the camera is allowed to arrive before enforcing the two-runner framing bounds, avoiding abrupt pullbacks at the old target. The finish still takes priority over an incomplete tour.

F9 stops the director and battle mode. Freecam remains manual and cancels a running tour. **Resume automatic Follow** enables the camera and exits Freecam; restart the tour explicitly if desired. F6 camera off resets the sequence. F10 also disables the director and coverage graphics. Exiting or rewinding a race resets the sequence.

## Automatic and saved finish cameras

Open **Automatic director > Finish coverage**. **Automatic finish coverage** works with either the director or manual Battle camera enabled. Set the distance-remaining trigger (default 400 m); known course length must exceed that threshold. With no course length, moving Follow/tour coverage continues and the panel requests an override instead of guessing the race end.

Finish coverage selects the current leader and P2 when within the battle maximum gap. An isolated leader uses a single-runner shot. The pair is held once either runner finishes, and no precomputed final order is used. **Front-quarter finish view** sets the moving angle; disable it to retain the operator's preferred angle.

With the director, **Fixed finish camera** and **Estimate finish position automatically** enabled, coverage initially follows the leaders and places a fixed camera in the last 120 m. Placement uses actual runner travel direction and course scale, positioning beside and beyond the estimated line. It then holds that world position and pans/zooms to track the selected contest. **Finish camera side** defaults to **Left of travel**, the opposite side from v0.1.22. It also sets the moving front-quarter angle, which looks back at the approaching runners. Choose **Right of travel** for the former side. Changing sides during an estimated fixed shot mirrors its lateral offset and eases across; its position along the finish straight stays fixed. Armed saved positions are used exactly as placed. Model rotation is not used for travel direction. If motion/scale data is unavailable, moving lead coverage continues with an explanatory status.

Automatic placement assumes a straight finish and may need adjustment for unusual course geometry. It does not inspect a finish-line transform or identify the final corner. To choose a precise spot:

1. Pause the race, enable Freecam and position beside and beyond the line, clear of the runners' path.
2. Choose **Save current Freecam as finish camera** inside **Finish coverage**. This stores and arms the position for the current race.
3. Return to Follow. An armed saved spot takes priority over automatic placement and starts at the configured finish trigger.
4. In a subsequent race, verify the course and choose **Arm saved finish camera for this race**. New races and replay rewinds disarm it. Only one saved position is stored.

The fixed camera retains ground protection. It uses the saved lens as its framing preference at 30 m, tightening at longer distances and widening as runners approach or separate. Estimated positions are discarded between races.

## Above-runner focus labels

Enable **On air > Runner focus markers** (on by default). The selected runner receives a blue **FOCUS** marker with gate and name. During a battle, both contenders are labeled **BATTLE / FOCUS** and **BATTLE / RIVAL**, with the rival in amber. These labels work in normal Follow, manual Battle and automatic directing without re-enabling the native arrows.

Markers project from the final camera after transitions and ground correction. Runners behind the camera or outside the viewport receive no marker. The default world anchor is now 2.1 units above the runner root (previously 2.9), with a 4-pixel connector gap (previously 14). **Marker height above ground** adjusts the anchor. Nearby labels separate horizontally where space allows; vertical stacking is a fallback, with connectors pointing to the runners. The labels ignore input, remain visible during F8 clean capture and disappear in Freecam, outside races or when camera control is off. F10 and **Hide coverage graphics** disable them; the preference saves automatically.

Projection was checked against the running game's Unity viewport coordinates. The v0.1.23 movement, pacing and finish-side changes still need in-game visual acceptance after installing the build. Director shot changes and final camera coordinates are recorded in the normal startup log to aid diagnosis.
