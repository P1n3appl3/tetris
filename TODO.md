## general features

- show combo counter/attack lines with gauge
- make rotation return whether or not a kick/spin happened (for different sound)

## ui

- headers for next piece queue and hold sections
- block skins, try "[]" like in tgm
- show lines remaining in sprint with background color

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
- choose next queue length
- setups from <https://four.lol>
  - give valid bags
  - track percent success

## visual references

- jstris
- <https://castur.itch.io/c-tetris>
- <https://akouzoukos.com/apotris>

## questions

- does rotating instantly trigger a shift if you have non-instant das already held? does it start the arr timer when you rotate, or is the arr timer completely independent
- how does tapping softdrop for 1 frame work? on jstris it seems kinda like it resets the
gravity delay when you release it, does this make it possible to infinitely suspend a piece
in midair?
- what's the highest you can go on a board with or without garbage?
