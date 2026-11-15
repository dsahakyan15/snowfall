# Snowfall

A snow overlay for the Windows desktop: snow falls over the screen, settles on windows and the bottom of the screen, and reacts to windows and the pointer.

## Language

### Snow

**Flake**:
One falling snow particle, before it settles.
_Avoid_: particle, snowflake sprite

**Pile**:
Settled snow on one surface, described by its height along that surface.
_Avoid_: drift, layer, heap, accumulation

**Snowball**:
A ball rolled out of a deep Pile by the pointer. It grows by taking snow from the Pile and rests on a Ledge or the Ground. It can be grabbed, carried and thrown; hard impacts break it back into loose snow that settles into Piles.

**Throw**:
Releasing a held Snowball in motion. It flies with the release velocity, gravity and a small push from the wind, and spins as it travels. A drop uses the same flight with no release velocity.

**Stack**:
Two Snowballs resting one above the other. A soft drop stacks on a resting ball up to 10% smaller; three balls become a Snowman only when their radii do not increase from bottom to top. Grabbing either ball separates the Stack.

**Snowman**:
Three stacked Snowballs, with non-increasing radii from bottom to top, joined into one entity with a coal face, carrot nose, red knitted scarf and buttons. It keeps their snow area, rides its Ledge, hops on fast window motion, and can be carried and thrown. Losing its Ledge lets it fall safely; a hard thrown Snowball breaks both back into loose snow.

**Ledge**:
The top edge of a window, which can carry a Pile.
_Avoid_: window top, shelf, surface

**Ground**:
The bottom edge of a screen, either the top of the taskbar or the screen bottom. It carries a Pile.
_Avoid_: floor, base

**Shed**:
A Pile leaving its Ledge: it slides off when the window moves, flies off when the window is Flung or shaken, and falls at once when the window closes or minimizes.
_Avoid_: drop, clear

**Fling**:
A fast window drag that throws the Pile off its Ledge into the air.
_Avoid_: throw, toss

**Melt**:
A Pile slowly losing height over time.
_Avoid_: thaw, decay

### Places

**Overlay**:
The see-through, click-through layer the snow is drawn on, one per screen.
_Avoid_: canvas, window (ambiguous with the user's windows)
