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
