#!/usr/bin/env python

from __future__ import annotations
from typing import List
from bs4 import BeautifulSoup
from dataclasses import dataclass
from datetime import timedelta, datetime, time
import matplotlib.pyplot as plt
from pathlib import Path
import pickle
import requests
import sys
import statistics


@dataclass
class Game:
    time: timedelta
    pieces: int
    pps: float
    faults: int
    date: datetime
    # TODO: optional replay link

    @staticmethod
    def from_row(row) -> Game:
        if ":" not in row[0]:
            row[0] = "0:" + row[0]
        if "." not in row[0]:
            row[0] = row[0] + ".0"
        t = datetime.strptime(row[0], "%M:%S.%f")
        return Game(
            t - datetime.combine(t.date(), time.min),
            int(row[1]),
            float(row[2]),
            int(row[3]),
            datetime.strptime(row[4], "%Y-%m-%d %H:%M:%S"),
        )


def get_games(user) -> List[Game]:
    path = Path(user + ".games")
    games = []

    if path.exists():
        print(f'Reading cached games, to grab fresh data delete "{path}"')
        with open(path, "rb") as f:
            games = pickle.load(f)
    else:
        url = f"https://jstris.jezevec10.com/sprint?display=5&user={user}&page="
        current = "0"
        while True:
            print("Getting next batch of times starting at:", current)
            response = requests.request("GET", url + current)
            soup = BeautifulSoup(response.text, "html.parser")
            rows = soup.table.find_all("tr")[1:]
            for row in rows:
                cols = [c.text.strip() for c in row.find_all("td")][2:-1]
                games.append(Game.from_row(cols))
            if len(rows) < 200:
                break
            current = str(games[-1].time.total_seconds() + 0.001)

        print("Got", len(games), "total games. Writing to file...")
        with open(path, "wb") as f:
            pickle.dump(games, f)

    return games

def graph_times(games):
    print("Here's a pretty graph:")
    games.sort(key=lambda g: g.date)
    times = [g.time.total_seconds() for g in games]
    # filter outliers
    stddev = statistics.stdev(times)
    mean = statistics.mean(times)
    threshold = 3
    times = list(filter(lambda t: t < mean + threshold * stddev, times))
    print("Filtered out", len(games) - len(times), "outliers")
    # linearly spaced looks better than time spaced
    points = list(enumerate(times))

    mins = [points[0]]
    sums = [0]
    avgs = []
    avgsn = []
    n = 50
    running_avg = 0
    for (i, time) in points:
        sums.append(sums[-1] + time)
        if i >= n:
            avgsn.append((i, (sums[-1] - sums[-(n + 1)]) / n))
        avgs.append((i, sums[-1] / (i + 1)))
        if time < mins[-1][-1]:
            mins.append((i, time))

    # plt.style.use('darcula')
    if Path("gruvbox").exists:
        plt.style.use('gruvbox')
    plt.scatter(*(zip(*points)), s=1, label="times")
    plt.plot(*(zip(*avgs)), label="rolling average")
    plt.plot(*(zip(*avgsn)), label=f"average of {n}")
    plt.plot(*(zip(*mins)), label="personal best")
    plt.xlabel("Games")
    plt.ylabel("Seconds")
    plt.legend()
    plt.grid(axis="x")
    plt.show()

def graph_faults(games):
    print("Here's a pretty graph:")
    games.sort(key=lambda g: g.date)
    faults = [g.faults for g in games]
    points = list(enumerate(faults))

    mins = [points[0]]
    sums = [0]
    avgs = []
    avgsn = []
    n = 50
    running_avg = 0
    for (i, time) in points:
        sums.append(sums[-1] + time)
        if i >= n:
            avgsn.append((i, (sums[-1] - sums[-(n + 1)]) / n))
        avgs.append((i, sums[-1] / (i + 1)))
        if time < mins[-1][-1]:
            mins.append((i, time))

    # plt.style.use('gruvbox')
    plt.scatter(*(zip(*points)), s=3, label="faults")
    plt.plot(*(zip(*avgs)), label="rolling average")
    plt.plot(*(zip(*avgsn)), label=f"average of {n}")
    plt.plot(*(zip(*mins)), label="personal best")
    plt.xlabel("Games")
    plt.ylabel("Faults")
    plt.legend()
    plt.grid(axis="x")
    plt.show()

if __name__ == "__main__":
    if not 1 < len(sys.argv) < 3:
        print("Usage:\n\t`python tetris.py <username>`")
        sys.exit(1)
    games = get_games(sys.argv[1])
    graph_times(games)
    # graph_faults(games)
