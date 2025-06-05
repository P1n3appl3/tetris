#!/usr/bin/env bash

# https://tetr.io/about/api/#usersuserrecordsgamemodeleaderboard
# can paginate with query param! ?after=(jq .data.entries[].p)

# xh https://ch.tetr.io/api/users/314neapple/records/40l/top?limit=100
# jq .data.entries[].results.stats.finaltime

# possibly use https://inoue.szy.lol/api for replay downloading
