#!/usr/bin/env python3

import vlc

class Player():
    def __init__(self):
        self.instance = vlc.Instance()
        self.mediaplayer = self.instance.media_player_new()

        self.media = self.instance.media_new("ep01.mkv")
        # put the media in the media player
        self.mediaplayer.set_media(self.media)

        # parse the metadata of the file
        self.media.parse()
        self.mediaplayer.play()

player = Player()

while True:
    pass
