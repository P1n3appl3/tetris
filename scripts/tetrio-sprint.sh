#!/usr/bin/env bash

user=$1
lines="${2-40}l"
page="https://ch.tetr.io/api/users/$user/records/$lines/top?limit=100"

f=$user-tetrio-sprints-$(date -I).txt
rm -f "$f"

# TODO: in theory you could use prisector with "recent" instead of "top" to do incremental updates?

# https://tetr.io/about/api/#usersuserrecordsgamemodeleaderboard
# maybe also useful:
# .results.stats.holds
# .results.stats.inputs
# .results.stats.clears.{singles,doubles,triples,quads}
# xh "$page" |
pri=""
sess="$RANDOM"

while [[ "$pri" != "&after=" ]]; do
    pri="&after=$(xh "$page$pri" "X-Session-ID:$sess" |
        tee /tmp/test.json |
        tee >(
            jq '.data.entries[] |
        (.results.stats.finaltime/1000,
            .results.stats.piecesplaced,
            .results.aggregatestats.pps,
            .results.stats.finesse.faults,
            .ts,
            .replayid)' -r |
                xargs -L6 |
                sd '\.\d{3}Z' '' >>"$f"
        ) | jq '.data.entries | last | .p[]' | sd '\n' ':' | head -c -1)"
    wc -l "$f"
done

sort -k5 "$f" | column -t | sponge "$f"

echo -e "\nrun:\n\ngnuplot -e \"filename='$f'\" sprints.gpi\n"
echo -e "for pretty graphs. check scripts.gpi for some more options or try\n"
echo -e "python chart-times.py $f"

# can use https://inoue.szy.lol/api for replay downloading
