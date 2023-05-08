#!/usr/bin/env python3

import json
import os

from mpris_server.events import EventAdapter
from mpris_server.server import Server

from player import Player

class Watchnext():
    def __init__(self):
        self.json_file_path = f"{os.path.expanduser('~')}/.config/watchnext/"
        try:
            with open(f"{self.json_file_path}/config", "rt", encoding="utf-8") as json_file:
                self.json_data = json.load(json_file)

        except FileNotFoundError:
            self.generate_config_file()

        self.select()

        self.player = Player(self)
        self.mpris = Server("Watchnext", adapter=self.player)
        event_handler = EventAdapter(root=self.mpris.root, player=self.mpris.player)
        self.player.register_event_handler(event_handler)

    def generate_config_file(self):
        print("Generating new config file...")
        os.mkdir(self.json_file_path)
        base_dir = input("Select a base directory: ")
        self.json_data = {
            "previous": None,
            "base_dir": base_dir,
            "series": {},
            "ignored_directories": [],
        }
        with open(f"{self.json_file_path}/config", "xt", encoding="utf-8") as json_file:
            json.dump(self.json_data, json_file, indent=2)

    def get_episodes(self, path):
        # Recurse?
        with os.scandir(path) as iterator:
            return sorted([entry.path for entry in iterator if entry.is_file()])

    def add_new_series(self):
        last_ignored = self.json_data["ignored_directories"]
        self.json_data["ignored_directories"] = []

        new_dirs = self.scan_for_new_dirs(self.json_data["base_dir"], last_ignored)

        if new_dirs:
            print(" -- New Directories! --")
            for entry in new_dirs:
                while True:
                    air = input(f"What to do for {entry} [Ignore/Add/Recurse]: ")
                    if air == "I":
                        self.json_data["ignored_directories"].append(entry)
                        break
                    if air == "A":
                        name = input("Name: ")
                        self.json_data["series"][name] = {"path": entry, "seen": 0, "tracks": None}
                        break
                    if air == 'R':
                        self.json_data["ignored_directories"].append(entry)
                        new_dirs += self.scan_for_new_dirs(entry, last_ignored)
                        break
        self.save()

    def scan_for_new_dirs(self, path, ignore):
        new_dirs = []
        with os.scandir(path) as iterator:
            for entry in iterator:
                if (not entry.name.startswith('.')
                    and entry.is_dir()
                    and entry.path not in
                    [self.json_data["series"][show]["path"] for show in self.json_data["series"]]):
                    if entry.path in ignore:
                        self.json_data["ignored_directories"].append(entry.path)
                    else:
                        new_dirs.append(entry.path)
        return new_dirs

    def select(self):
        self.add_new_series()

        print("Series: ")
        tmp_series_map = []
        for i, series in enumerate(sorted(self.json_data["series"])):
            path = self.json_data["series"][series]["path"]
            episodes = self.get_episodes(path)
            try:
                index = self.json_data["series"][series]["seen"]
                print(f"  {i+1}: {series} - E{index+1}")
                tmp_series_map.append((series, index, episodes))
            except IndexError:
                continue

        while True:
            try:
                previous = self.json_data["previous"]
                selected = input(f"Which one to watch next? [{previous}]: ")
                if selected == '':
                    if not self.json_data["previous"][0]:
                        print("No previous series!")
                        continue
                    for series, index, episodes in tmp_series_map:
                        if series == previous:
                            selected = (series, index, episodes)
                else:
                    selected = int(selected)
                    if selected < 1:
                        raise IndexError("list index out of range")
                    selected = tmp_series_map[selected-1]
                    self.json_data["previous"] = selected[0]

                self.series, self.index, self.episodes = selected
                return

            except KeyboardInterrupt:
                return
            except Exception as err:
                print(f"Error: {err}")

    def evaluate_progress(self, progress):
        if progress > 0.9:
            self.json_data["series"][self.series]["seen"] += 1
            self.save()

    def next(self):
        self.index += 1
        self.json_data["series"][self.series]["seen"] += 1
        self.save()

        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")
        self.player.play()

        audio, subs = self.json_data["series"][self.series]["tracks"]
        self.player.set_tracks(audio, subs)

    def previous(self):
        self.index -= 1
        self.json_data["series"][self.series]["seen"] -= 1
        self.save()

        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")
        self.player.play()

        audio, subs = self.json_data["series"][self.series]["tracks"]
        self.player.set_tracks(audio, subs)

    def start(self):
        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")
        self.player.mediaplayer.toggle_fullscreen()
        self.player.create_window()

        if ("tracks" not in self.json_data["series"][self.series]
            or self.json_data["series"][self.series]["tracks"] is None):
            audios, subss = self.player.get_tracks()
            print("Audio tracks:")
            for audio in audios:
                print(f"  {audio[0]}: {audio[1].decode('ascii')}")
            audio = int(input("Audio track: "))
            print("Subtitle tracks:")
            for subs in subss:
                print(f"  {subs[0]}: {subs[1].decode('ascii')}")
            subs = int(input("Subtitle track: "))

            self.json_data["series"][self.series]["tracks"] = (audio, subs)
            self.save()

        audio, subs = self.json_data["series"][self.series]["tracks"]
        self.player.set_tracks(audio, subs)

        self.mpris.loop()

    def save(self):
        with open(f"{self.json_file_path}/config", "wt", encoding="utf-8") as json_file:
            json.dump(self.json_data, json_file, indent=2)


watchnext = Watchnext()
try:
    watchnext.start()
except KeyboardInterrupt:
    watchnext.player.stop()
