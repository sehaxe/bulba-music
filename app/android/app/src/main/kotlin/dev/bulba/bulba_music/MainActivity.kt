package dev.bulba.bulba_music

import com.ryanheise.audioservice.AudioServiceActivity

// audio_service owns the media session, so the activity has to be its subclass:
// it wires the service to the widget tree and keeps the session alive while the
// task is in the background.
class MainActivity : AudioServiceActivity()
