#!/usr/bin/env python

from __future__ import annotations
from dataclasses import dataclass
from datetime import timedelta, datetime, time
import matplotlib.pyplot as plt
from pathlib import Path
import sys
import statistics


@dataclass
class Game:
    time: timedelta
    pieces: int
    pps: float
    faults: int
    date: datetime
    replay: int

    @staticmethod
    def from_row(row) -> Game:
        return Game(
            timedelta(seconds=float(row[0])),
            int(row[1]),
            float(row[2]),
            int(row[3]),
            datetime.strptime(row[4], "%Y-%m-%dT%H:%M:%S"),
            int(row[5]) if row[5] != "null" else 0,
        )


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
    for i, time in points:
        sums.append(sums[-1] + time)
        if i >= n:
            avgsn.append((i, (sums[-1] - sums[-(n + 1)]) / n))
        avgs.append((i, sums[-1] / (i + 1)))
        if time < mins[-1][-1]:
            mins.append((i, time))

    if Path("gruvbox").exists():
        plt.style.use("gruvbox")
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
    for i, time in points:
        sums.append(sums[-1] + time)
        if i >= n:
            avgsn.append((i, (sums[-1] - sums[-(n + 1)]) / n))
        avgs.append((i, sums[-1] / (i + 1)))
        if time < mins[-1][-1]:
            mins.append((i, time))

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
        print("Usage:\n\t`python chart-times.py <username>.games`")
        sys.exit(1)
    games = []
    for line in open(sys.argv[-1]):
        games.append(Game.from_row(line.split()))

    graph_times(games)
    # graph_faults(games)
