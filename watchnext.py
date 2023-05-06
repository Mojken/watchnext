#!/usr/bin/env python3

import vlc
import json
import os

from typing import Optional
from decimal import Decimal

from mpris_server.adapters import MprisAdapter, Track, PlayState
from mpris_server.events import EventAdapter
from mpris_server.server import Server
from mpris_server import Metadata

class Player(MprisAdapter):
    def __init__(self, watchnext):
        self.watchnext = watchnext
        self.instance = vlc.Instance()
        self.mediaplayer = self.instance.media_player_new()

        self.media = self.instance.media_new(self.watchnext.path)
        # put the media in the media player
        self.mediaplayer.set_media(self.media)

        # parse the metadata of the file
        self.media.parse()
        self.mediaplayer.play()

    def get_current_track(self) -> Track:
        print("get_current_track")
        return Track(
            artists=(),
            length=self.mediaplayer.get_length() * 1000,
            name=self.watchnext.filename,
        )

    def get_current_position(self) -> int:
        print("get_current_position")
        return self.mediaplayer.get_time()

    def next(self):
        print("Unimplemented: next")
        pass # TODO: implement

    def previous(self):
        print("Unimplemented: previous")
        pass # TODO: implement

    def pause(self):
        print("pause")
        self.mediaplayer.pause()

    def resume(self):
        print("resume")
        self.play()

    def stop(self):
        progress = self.mediaplayer.get_position()
        print(f"stopping at {progress}")
        self.watchnext.evaluate_progress(progress)
        self.mediaplayer.stop()

    def play(self):
        print("play")
        self.mediaplayer.play()

    def get_playstate(self) -> PlayState:
        state = self.mediaplayer.get_state()
        print(f"get_playstate: {state}")
        if state == vlc.State.Playing:
            return PlayState.PLAYING
        if state == vlc.State.Paused:
            return PlayState.PAUSED
        if state == vlc.State.Stopped:
            return PlayState.STOPPED
        print("Invalid state?")

    def seek(self, time: int, track_id: Optional[str] = None):
        time /= 1000
        print(f"seek: {time} / {self.mediaplayer.get_length()}")
        self.mediaplayer.set_time(int(time))

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
        return self.watchnext.filename
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
        self.json_file_path = "/anime/.watchnext.data"
        with open(self.json_file_path) as json_file:
            self.json_data = json.load(json_file)

        self.select()
        self.save()

    def select(self):
        last_ignored = self.json_data["ignored_directories"]
        self.json_data["ignored_directories"] = []
        new_dirs = []

        with os.scandir("/anime") as iterator:
            for entry in iterator:
                if not entry.name.startswith('.') and entry.is_dir() and entry.name not in [self.json_data["anime"][show]["path"] for show in self.json_data["anime"]]:
                    if entry.name in last_ignored:
                        self.json_data["ignored_directories"].append(entry.name)
                    else:
                        new_dirs.append(entry.name)

        if new_dirs != []:
            print(" -- New Directories! --")
            for entry in new_dirs:
                while True:
                    ir = input(f"What to do for {entry} [Ignore/Register]: ")
                    if ir == "I":
                        self.json_data["ignored_directories"].append(entry)
                        break
                    elif ir == "R":
                        name = input("Name: ")
                        self.json_data["anime"][name] = {"path": entry, "seen": 0, "subtrack": None, "audiotrack": None}
                        break

        print(f"Previous: {self.json_data['previous']}")
        print("All: ")
        tmp_series_map = []
        finished_series = None
        for n, series in enumerate(self.json_data["anime"]):
            path = self.json_data["anime"][series]["path"]
            episodes = sorted(os.listdir(f"/anime/{path}"))
            try:
                filename = episodes[self.json_data["anime"][series]["seen"]]
                print(f"  {n+1}: {series} - {filename}")
                tmp_series_map.append((f"/anime/{path}/{filename}", series, filename))

            except IndexError:
                finished_series = series

        if finished_series:
            series_data = self.json_data["anime"].pop(finished_series)
            self.json_data["ignored_directories"].append(series_data["path"])
            self.json_data['previous'] = (None, "")

        while True:
            try:
                selected = input(f"Which one to watch next? [{self.json_data['previous'][1]}]")
                if selected == '':
                    if not self.json_data["previous"][0]:
                        print("No previous anime!")
                        continue
                    self.path, self.series, self.filename = self.json_data["previous"]
                    return

                selected = int(selected)
                if selected < 1:
                    raise IndexError("list index out of range")
                selected = tmp_series_map[selected-1]
                self.json_data["previous"] = selected
                self.path, self.series, self.filename = selected
                return

            except KeyboardInterrupt:
                return
            except Exception as e:
                print("Error: ")
                print(e)

    def evaluate_progress(self, progress):
        if progress > 0.9:
            self.json_data["anime"][self.series]["seen"] += 1

    def start(self, series = None):
        self.player = Player(self)
        self.mpris = Server("Watchnext", adapter=self.player)
        self.event_handler = EventAdapter(root=self.mpris.root, player=self.mpris.player)

        self.mpris.loop()

    def save(self):
        with open(self.json_file_path, "w") as json_file:
            json.dump(self.json_data, json_file)

watchnext = Watchnext()
try:
    watchnext.start()
except KeyboardInterrupt:
    pass
except Exception as e:
    print("Error: ")
    print(e)
finally:
    watchnext.save()
