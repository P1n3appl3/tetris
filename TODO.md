## general features

- show combo counter/attack lines with gauge
- make rotation return whether or not a kick/spin happened (for different sound)

## ui

- background grid in light grey and black
  - does ghost piece overlay or occlude?
- headers for next piece queue and hold sections
- block skins, try "[]" like in tgm
- show lines remaining in sprint with background color

### web

- try trunkrs.dev for development

## practice

- undo key
- finesse
- follow replay (show ghost piece)
- combo (4 wide like apotris)
  - rewind when made impossible
  - <https://ddrkirby.com/games/4-wide-trainer/4-wide-trainer.html>
- full-clear
  - rewind when made impossible
- tspin setups (optional show ghost piece)
  - <https://github.com/himitsuconfidential/downstack-practice>
- openers
  - <https://blox.askplays.com/?opener=Mountainous+Stacking+2>
    - side note: the way they do lock delay is really cool: start with big timer that's
      shown as bottom screen bar, then every rotation increase the starting point of that bar
- choose next queue length
- setups from <https://four.lol>
  - give valid bags
  - track percent success
- cheese race

## visual references

- jstris
- <https://castur.itch.io/c-tetris>
- <https://akouzoukos.com/apotris>

## questions

- does rotating instantly trigger a shift if you have non-instant das already held? does it
  start the arr timer when you rotate, or is the arr timer completely independent
- if I'm buffering das while das-ing in the other direction, when you release that direction
  do we instantly trigger an ARR, or do we restart the ARR timer?
- how does tapping softdrop for 1f/a short time work? on jstris it seems kinda like it resets the
  gravity delay when you release it, does this make it possible to infinitely suspend a piece
  in midair?
- what's the highest you can go on a board with or without garbage?
