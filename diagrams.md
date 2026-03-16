# Space Invaders — Architecture Diagrams

## 1. Game Phase State Machine

```mermaid
stateDiagram-v2
    [*] --> Attract : app start

    Attract --> Playing : Space

    Playing --> Paused    : P
    Paused  --> Playing   : P
    Paused  --> Attract   : Q

    Playing --> LevelClear : all aliens dead\n+ no UFO on screen
    LevelClear --> Playing : pause timer expires\n(advance_level)

    Playing --> GameOver : invasion reaches ship\nor lives reach 0
    GameOver --> Attract : Space\n(after GAME_OVER_PAUSE)
```

---

## 2. Per-Frame Update Pipeline

```mermaid
flowchart TD
    A[requestAnimationFrame] --> B[Read keyboard state]
    B --> C{Phase?}

    C -->|Attract| D[Space → reset_game\nPhase = Playing]
    C -->|GameOver| E[Space → reset_game\nafter pause]
    C -->|Paused| F[P → resume\nQ → quit to Attract]

    C -->|Playing| G[move_ship]
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

```mermaid
xychart-beta
    title "Tick interval vs aliens alive (lower = faster)"
    x-axis "Aliens alive" [1, 10, 20, 30, 40, 55]
    y-axis "Tick interval (frames)" 0 --> 32
    line "Level 1 (scale 1.00)" [4, 7, 12, 17, 22, 30]
    line "Level 2 (scale 0.75)" [4, 5,  9, 13, 16, 22]
    line "Level 3 (scale 0.55)" [4, 4,  7,  9, 12, 16]
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

| Level | fire_interval | speed_scale | grid_y_offset | max_bullets | ufo_first |
|-------|--------------|-------------|---------------|-------------|-----------|
| 1     | 90           | 1.00        | 0             | 3           | 23        |
| 2     | 65           | 0.75        | 1×CELL_H      | 3           | 20        |
| 3     | 45           | 0.55        | 2×CELL_H      | 3           | 15        |
| 4     | 38           | 0.50        | 2×CELL_H      | **4**       | 12        |
| 5     | 32           | 0.45        | 3×CELL_H      | 4           | 10        |
| 6     | 27           | 0.40        | 3×CELL_H      | **5**       | 8         |
| 7     | 23           | 0.36        | 4×CELL_H      | 5           | 7         |
| 8     | 20           | 0.32        | 4×CELL_H      | **6**       | 6         |
| 9     | 17           | 0.29        | 4×CELL_H      | 6           | 5         |
| 10    | 15           | 0.25        | 4×CELL_H      | **7**       | 4         |
