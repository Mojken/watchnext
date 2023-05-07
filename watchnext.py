#!/usr/bin/env python3

import time
import vlc
import json
import os

from typing import Optional
from decimal import Decimal

from mpris_server.adapters import MprisAdapter, Track, PlayState
from mpris_server.events import EventAdapter
from mpris_server.server import Server

BASE_DIR = "/anime"

class Player(MprisAdapter):
    def __init__(self, watchnext):
        self.watchnext = watchnext
        self.instance = vlc.Instance()
        self.mediaplayer = self.instance.media_player_new()

    def set_file(self, path, name):
        self.name = name
        self.media = self.instance.media_new(path)
        self.mediaplayer.set_media(self.media)
        self.media.parse()

    def get_tracks(self):
        return (self.mediaplayer.audio_get_track_description(), self.mediaplayer.video_get_spu_description())

    def set_tracks(self, audio, subs):
        self.mediaplayer.audio_set_track(audio)
        self.mediaplayer.video_set_spu(subs)

    def get_current_track(self) -> Track:
        print("get_current_track")
        return Track(
            artists=(),
            length=self.mediaplayer.get_length() * 1000,
            name=self.name,
        )

    def get_current_position(self) -> int:
        ret = self.mediaplayer.get_time()
        print(f"get_current_position {ret}")
        return ret * 1000

    def next(self):
        self.watchnext.next()

    def previous(self):
        self.watchnext.previous()

    def pause(self):
        print("pause")
        self.mediaplayer.pause()
        time.sleep(0.1)
        self.watchnext.event_handler.on_playpause()

    def resume(self):
        print("resume")
        self.play()

    def stop(self):
        progress = self.mediaplayer.get_position()
        print(f"stopping at {progress}")
        self.watchnext.evaluate_progress(progress)
        self.mediaplayer.stop()
        time.sleep(0.1)
        self.watchnext.event_handler.on_ended()

    def play(self):
        print("play")
        self.mediaplayer.play()
        time.sleep(0.1)
        self.watchnext.event_handler.on_playpause()

    def get_playstate(self) -> PlayState:
        state = self.mediaplayer.get_state()
        print(f"get_playstate: {state}")
        if state == vlc.State.Playing:
            return PlayState.PLAYING
        if state == vlc.State.Paused:
            return PlayState.PAUSED
        if state == vlc.State.Stopped:
            return PlayState.STOPPED
        if state == vlc.State.Ended:
            self.watchnext.event_handler.on_ended()
            return PlayState.STOPPED
        print(f"Unhandled State {state}")
        return PlayState.STOPPED

    def seek(self, ts: int, track_id: Optional[str] = None):
        modtime = int(ts / 1000)
        print(f"seek: {ts} ({track_id})")
        self.mediaplayer.set_time(modtime)

        time.sleep(0.1)
        self.watchnext.event_handler.on_seek(modtime * 1000)

    def open_uri(self, uri: str):
        print("Unimplemented: open_uri")
        pass # TODO: implement

    def is_repeating(self) -> bool:
        print("is_repeating")
        return False

    def is_playlist(self) -> bool:
        print("is_playlist")
        return True

    def set_repeating(self, val: bool):
        print("Unimplemented: set_repeating")
        pass # TODO: implement

    def set_loop_status(self, val: str):
        print("Unimplemented: set_loop_status")
        pass # TODO: implement

    def get_rate(self) -> Decimal:
        print("get_rate")
        return 1.0

    def set_rate(self, val: Decimal):
        print("Unimplemented: set_rate")
        pass # TODO: implement

    def set_minimum_rate(self, val: Decimal):
        print("Unimplemented: set_minimum_rate")
        pass # TODO: implement

    def set_maximum_rate(self, val: Decimal):
        print("Unimplemented: set_maximum_rate")
        pass # TODO: implement

    def get_minimum_rate(self) -> Decimal:
        print("get_minimum_rate")
        return 0.0

    def get_maximum_rate(self) -> Decimal:
        print("get_maximum_rate")
        return 1.0

    def get_shuffle(self) -> bool:
        print("get_shuffle")
        return False

    def set_shuffle(self, val: bool):
        print("Unimplemented: set_shuffle")
        pass # TODO: implement

    def get_volume(self) -> Decimal:
        ret = vlc.libvlc_audio_get_volume(self.mediaplayer)
        print(f"get_volume: {ret}%")
        return ret / 150

    def set_volume(self, val: Decimal):
        val = int(val * 150)
        print(f"set_volume: {val}%")
        vlc.libvlc_audio_set_volume(self.mediaplayer, val)
        time.sleep(0.1)
        self.watchnext.event_handler.on_volume()

    def is_mute(self) -> bool:
        ret = bool(vlc.libvlc_audio_get_mute(self.mediaplayer))
        print(f"is_mute {ret}")
        return ret

    def set_mute(self, val: bool):
        print(f"set_mute: {val}")
        vlc.libvlc_audio_set_mute(self.mediaplayer, val)
        pass

    def can_go_next(self) -> bool:
        print("can_go_next")
        return True

    def can_go_previous(self) -> bool:
        print("can_go_previous")
        return True

    def can_play(self) -> bool:
        print("can_play")
        return self.can_control()

    def can_pause(self) -> bool:
        print("can_pause")
        return self.can_control()

    def can_seek(self) -> bool:
        print("can_seek")
        return True

    def can_control(self) -> bool:
        print("can_control")
        return True

    def get_stream_title(self) -> str:
        return self.name
        pass # TODO: implement

    def get_previous_track(self) -> Track:
        print("Unimplemented: get_previous_track")
        pass # TODO: implement

    def get_next_track(self) -> Track:
        print("Unimplemented: get_next_track")
        pass # TODO: implement

    def __getattr__(self, name):
        def method(*args):
            print(f"Unknown method: {name}")
            if args:
                print(str(args))

            return method


class Watchnext():
    def __init__(self):
        self.json_file_path = f"{os.path.expanduser('~')}/.config/watchnext/"
        try:
            with open(f"{self.json_file_path}/config", "r") as json_file:
                self.json_data = json.load(json_file)

        except FileNotFoundError:
            self.generate_config_file()

        self.select()
        self.save()

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
        with open(f"{self.json_file_path}/config", "x") as json_file:
            json.dump(self.json_data, json_file, indent=2)

    def get_episodes(path):
        # Recurse?
        with os.scandir(path) as iterator:
            return sorted([entry.path for entry in iterator if entry.is_file()])

    def add_new_series(self):
        self.last_ignored = self.json_data["ignored_directories"]
        self.json_data["ignored_directories"] = []

        new_dirs = self.scan_for_new_dirs(self.json_data["base_dir"])

        if new_dirs != []:
            print(" -- New Directories! --")
            for entry in new_dirs:
                while True:
                    air = input(f"What to do for {entry} [Ignore/Add/Recurse]: ")
                    if air == "I":
                        self.json_data["ignored_directories"].append(entry)
                        break
                    elif air == "A":
                        name = input("Name: ")
                        self.json_data["series"][name] = {"path": entry, "seen": 0, "tracks": None}
                        break
                    elif air == 'R':
                        self.json_data["ignored_directories"].append(entry)
                        new_dirs += self.scan_for_new_dirs(entry)
                        break

    def scan_for_new_dirs(self, path):
        new_dirs = []
        with os.scandir(path) as iterator:
            for entry in iterator:
                if not entry.name.startswith('.') and entry.is_dir() and entry.path not in [self.json_data["series"][show]["path"] for show in self.json_data["series"]]:
                    if entry.path in self.last_ignored:
                        self.json_data["ignored_directories"].append(entry.path)
                    else:
                        new_dirs.append(entry.path)
        return new_dirs

    def select(self):
        self.add_new_series()
        print("Series: ")
        tmp_series_map = []
        for n, series in enumerate(sorted(self.json_data["series"])):
            path = self.json_data["series"][series]["path"]
            episodes = Watchnext.get_episodes(path)
            try:
                index = self.json_data["series"][series]["seen"]
                print(f"  {n+1}: {series} - E{index+1}")
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
            except Exception as e:
                print("Error: ")
                print(e)

    def evaluate_progress(self, progress):
        if progress > 0.9:
            self.json_data["series"][self.series]["seen"] += 1

    def next(self):
        self.index += 1;
        self.json_data["series"][self.series]["seen"] += 1;
        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")
        self.player.play()
        self.event_handler.on_title()

    def previous(self):
        self.index -= 1;
        self.json_data["series"][self.series]["seen"] -= 1;
        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")
        self.player.play()
        self.event_handler.on_title()

    def start(self, series = None):
        self.player = Player(self)
        self.mpris = Server("Watchnext", adapter=self.player)
        self.event_handler = EventAdapter(root=self.mpris.root, player=self.mpris.player)

        self.player.set_file(self.episodes[self.index], f"{self.series} - E{self.index+1}")

        self.player.mediaplayer.toggle_fullscreen()

        # Start renderer, wait until state changes, pause playback
        # There might be an actual function for opening the window but not starting playback?
        self.player.play()
        while self.player.mediaplayer.get_state() != vlc.State.Playing:
            pass
        time.sleep(0.1)
        self.player.pause()

        if "tracks" not in self.json_data["series"][self.series] or self.json_data["series"][self.series]["tracks"] is None:
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
        with open(f"{self.json_file_path}/config", "w") as json_file:
            json.dump(self.json_data, json_file, indent=2)

watchnext = Watchnext()
try:
    watchnext.start()
except KeyboardInterrupt:
    pass
except Exception as e:
    print("Error: ")
    print(e)
finally:
    watchnext.player.stop()
    watchnext.save()
