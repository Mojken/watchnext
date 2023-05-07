#!/usr/bin/env python3

from typing import Optional
from decimal import Decimal
from time import sleep

import dbus

import gi
gi.require_version('Gtk', '3.0')
from gi.repository import GLib, Gtk

import vlc

from mpris_server.adapters import MprisAdapter, Track, PlayState


class Window(Gtk.DrawingArea):
    def __init__(self, player):
        Gtk.DrawingArea.__init__(self)
        self.player = player

        def handle_embed(*args):
            self.player.set_xwindow(self.get_window().get_xid())
            return True

        self.connect("realize", handle_embed)

        # Try to keep the screen awake
        try:
            self.screensaver_interface = dbus.Interface(
                dbus.SessionBus().get_object(
                    "org.freedesktop.ScreenSaver",
                    "/org/freedesktop/ScreenSaver"
                ),
                dbus_interface="org.freedesktop.ScreenSaver",
            )
        except dbus.exceptions.DBusException:
            self.screensaver_interface = False
        self.dbus_inhibit = None

    def set_keepawake(self):
        if self.screensaver_interface:
            self.dbus_inhibit = self.screensaver_interface.Inhibit("watchnext", "vlc is playing")

    def unset_keepawake(self):
        if self.dbus_inhibit:
            self.screensaver_interface.UnInhibit(self.dbus_inhibit)
            self.dbus_inhibit = None

class Player(MprisAdapter):
    def __init__(self, watchnext_ref):
        super(MprisAdapter, self).__init__()

        self.watchnext = watchnext_ref
        self.instance = vlc.Instance()
        self.mediaplayer = self.instance.media_player_new()

        self.name = ""
        self.media = None
        self.event_handler = None

        self.window = Window(self.mediaplayer)

        w = Gtk.Window()
        w.add(self.window)
        w.show_all()
        w.connect("destroy", Gtk.main_quit)

    def register_event_handler(self, event_handler):
        self.event_handler = event_handler

    def create_window(self):
        # Start renderer, wait until state changes, pause playback
        # There might be an actual function for opening the window but not starting playback?
        self.play()
        while self.mediaplayer.get_state() != vlc.State.Playing:
            pass
        sleep(0.1)
        self.pause()

    def set_file(self, path, name):
        self.name = name
        self.media = self.instance.media_new(path)
        self.mediaplayer.set_media(self.media)
        self.media.parse()

    def get_tracks(self):
        return (
            self.mediaplayer.audio_get_track_description(),
            self.mediaplayer.video_get_spu_description()
        )

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
        self.play()
        self.event_handler.on_title()

    def previous(self):
        self.watchnext.previous()
        self.play()
        self.event_handler.on_title()

    def pause(self):
        print("pause")
        self.mediaplayer.pause()
        sleep(0.1)
        self.event_handler.on_playpause()
        self.window.unset_keepawake()

    def resume(self):
        print("resume")
        self.play()

    def stop(self):
        progress = self.mediaplayer.get_position()
        print(f"stopping at {progress}")
        self.watchnext.evaluate_progress(progress)
        self.mediaplayer.stop()
        sleep(0.1)
        self.event_handler.on_ended()
        self.window.unset_keepawake()

    def play(self):
        print("play")
        self.mediaplayer.play()
        sleep(0.1)
        self.event_handler.on_playpause()
        self.window.set_keepawake()

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
            self.event_handler.on_ended()
            return PlayState.STOPPED
        print(f"Unhandled State {state}")
        return PlayState.STOPPED

    def seek(self, time: int, track_id: Optional[str] = None):
        modtime = int(time / 1000)
        print(f"seek: {time} ({track_id})")
        self.mediaplayer.set_time(modtime)

        sleep(0.1)
        self.event_handler.on_seek(modtime * 1000)

    def is_repeating(self) -> bool:
        print("is_repeating")
        return False

    def is_playlist(self) -> bool:
        print("is_playlist")
        return True

    def get_rate(self) -> Decimal:
        print("get_rate")
        return 1.0

    def get_minimum_rate(self) -> Decimal:
        print("get_minimum_rate")
        return 0.0

    def get_maximum_rate(self) -> Decimal:
        print("get_maximum_rate")
        return 1.0

    def get_shuffle(self) -> bool:
        print("get_shuffle")
        return False

    def get_volume(self) -> Decimal:
        ret = vlc.libvlc_audio_get_volume(self.mediaplayer)
        print(f"get_volume: {ret}%")
        return ret / 150

    def set_volume(self, val: Decimal):
        val = int(val * 150)
        print(f"set_volume: {val}%")
        vlc.libvlc_audio_set_volume(self.mediaplayer, val)
        sleep(0.1)
        self.event_handler.on_volume()

    def is_mute(self) -> bool:
        ret = bool(vlc.libvlc_audio_get_mute(self.mediaplayer))
        print(f"is_mute {ret}")
        return ret

    def set_mute(self, val: bool):
        print(f"set_mute: {val}")
        vlc.libvlc_audio_set_mute(self.mediaplayer, val)

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

    def __getattr__(self, name):
        def method(*args):
            print(f"Unknown method: {name}")
            if args:
                print(str(args))

            return method
