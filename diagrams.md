# Space Invaders — Architecture Diagrams

## 1. Game Phase State Machine

```mermaid
stateDiagram-v2
    [*] --> Attract : app start

    Attract --> Playing    : Space
    Attract --> Scoreboard : H

    Scoreboard --> Attract : H

    Playing --> Paused    : P
    Paused  --> Playing   : P
    Paused  --> Attract   : Q

    Playing --> LevelClear : all aliens dead\n+ no UFO on screen
    LevelClear --> Playing : pause timer expires\n(advance_level)

    Playing --> GameOver : invasion reaches ship\nor lives reach 0

    GameOver --> NameEntry : Space (after pause)\nif score qualifies
    GameOver --> Attract   : Space (after pause)\nif score doesn't qualify

    NameEntry --> Attract : Enter (save)\nor Escape (skip)
```

---

## 2. Per-Frame Update Pipeline

```mermaid
flowchart TD
    A[requestAnimationFrame] --> B[Read keyboard state]
    B --> C{Phase?}

    C -->|Attract| D[Space → reset_game\nH → open_scoreboard]
    C -->|Scoreboard| SC[H → close_scoreboard]
    C -->|GameOver| E[Space → NameEntry or Attract\nafter GAME_OVER_PAUSE]
    C -->|Paused| F[P → resume\nQ → quit to Attract]
    C -->|NameEntry| NE[Enter → submit_name\nEscape → skip\nBackspace / chars → edit buffer]

    C -->|Playing| G[move_ship\nS → toggle sound]
    G --> H[fire / try_spawn_ufo]
    H --> I[step_bullet]
    I --> J[check_ufo_hit]
    J --> K[check_bullet_hit aliens]
    K --> L[step_grid]
    L --> M[check_alien_hit_ship]
    M --> N[step_alien_bullets]
    N --> O[fire_alien_bullet\nevery N frames]
    O --> P[check_invasion]
    P --> Q[check_level_clear]

    Q --> R[tick_level_clear]
    R --> S[tick_game_over]
    S --> T[tick_explosions]
    T --> U[tick_ufo]
    U --> V[draw_scene]
    V --> W[PostProcessor::process\nCRT WebGL pass]
```

---

## 3. Level Specification Architecture

```mermaid
flowchart LR
    subgraph Spec ["LevelSpec (static data)"]
        P[pattern: LevelPattern\nalien type grid]
        F[alien_fire_interval]
        SS[speed_scale]
        GY[grid_y_offset]
        U1[ufo_first_shot]
        U2[ufo_repeat_shots]
        MB[max_alien_bullets]
    end

    subgraph LEVELS ["LEVELS array"]
        L1[LevelSpec × Level 1]
        L2[LevelSpec × Level 2]
        L3[LevelSpec × Level 3]
    end

    subgraph State ["GameState (runtime)"]
        SA[aliens: Vec&lt;Alien&gt;]
        SF[alien_fire_interval: u32]
        SSS[speed_scale: f64]
        SG[grid.offset_y: f64]
        SU1[ufo_shots_to_next: u32]
        SLI[level: usize]
    end

    L1 & L2 & L3 --> LEVELS
    LEVELS -->|advance_level\nreset_game| State

    State -->|speed_scale| CS[ClassicSpeed\ncontrols tick_interval]
    State -->|alien_fire_interval| Loop[Game loop\nfire_alien_bullet cadence]
    State -->|max_alien_bullets| FAB[fire_alien_bullet\ncap]
    State -->|ufo_shots_to_next| UFO[try_spawn_ufo]
```

---

## 4. Classic Speed Curve

`ClassicSpeed` maps alive alien count → tick interval (frames between grid moves).
`speed_scale < 1.0` compresses the curve downward — the grid is faster throughout.
`speed_scale > 1.0` stretches it upward — used on level 1 to ease the player in.

```mermaid
xychart-beta
    title "Tick interval vs aliens alive (lower = faster)"
    x-axis "Aliens alive" [1, 10, 20, 30, 40, 55]
    y-axis "Tick interval (frames)" 0 --> 40
    line "Level 1 (scale 1.20)" [5, 9, 15, 21, 27, 36]
    line "Level 3 (scale 0.80)" [4, 6, 10, 14, 18, 24]
    line "Level 6 (scale 0.50)" [4, 4,  6,  9, 11, 15]
```

---

## 5. Level Grid Patterns & Difficulty Table

Each cell is one alien. Row 0 is the top of the formation.

```
Level 1         Level 2         Level 3         Level 4
S S S S S S S   S S S S S S S   S S S S S S S   S S S S S S S
C C C C C C C   S S S S S S S   S S S S S S S   S S S S S S S
C C C C C C C   C C C C C C C   S S S S S S S   S S S S S S S
O O O O O O O   O O O O O O O   C C C C C C C   C C C C C C C
O O O O O O O   O O O O O O O   O O O O O O O   C C C C C C C

Level 5         Level 6         Level 7
S S S S S S S   S S S S S S S   S S S S S S S
C S C S C S C   S S S S S S S   S S S S S S S
S S S S S S S   S C S C S C S   S S S S S S S
C S C S C S C   S S S S S S S   S S S S S S S
S S S S S S S   S S S S S S S   C C C C C C C

Levels 8-10 recycle patterns 6/7/6 at maximum difficulty settings.
S=Squid (30pts)  C=Crab (20pts)  O=Octopus (10pts)
```

`CELL_H = 48px`. `grid_y_offset` shifts the formation down from its default starting row.

| Level | fire_interval | speed_scale | grid_y_offset | max_bullets | ufo_first | ufo_repeat |
|-------|--------------|-------------|---------------|-------------|-----------|------------|
| 1     | 90           | 1.20        | 0             | 3           | 23        | 15         |
| 2     | 65           | 0.95        | 0             | 3           | 20        | 12         |
| 3     | 45           | 0.80        | 1×CELL_H      | 3           | 15        | 10         |
| 4     | 38           | 0.70        | 1×CELL_H      | **4**       | 12        | 8          |
| 5     | 32           | 0.60        | 2×CELL_H      | 4           | 10        | 7          |
| 6     | 27           | 0.50        | 2×CELL_H      | **5**       | 8         | 6          |
| 7     | 23           | 0.45        | 3×CELL_H      | 5           | 7         | 5          |
| 8     | 20           | 0.40        | 3×CELL_H      | **6**       | 6         | 4          |
| 9     | 17           | 0.36        | 3×CELL_H      | 6           | 5         | 3          |
| 10    | 15           | 0.32        | 4×CELL_H      | **7**       | 4         | 3          |
