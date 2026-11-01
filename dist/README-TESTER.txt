WISP 0.1.8 FOR WINDOWS
======================

Wisp is a Dynamic Island-style notch for Windows 10/11. It sits at the top
center of your main screen.


1. FONT
-------
Wisp is designed for Apple's SF Pro Display. If it's installed on your PC,
Wisp uses it. If not, Wisp uses Inter, which is built into wisp.exe, so it
looks right either way and nothing gets installed.

Want the exact original look? SF Pro is a free download from Apple:
  a. Go to https://developer.apple.com/fonts/ and download "SF Pro".
  b. The download is a .dmg file. Open it with 7-Zip (https://7-zip.org):
     right-click > 7-Zip > Open archive. Inside, open "SF Pro Fonts.pkg",
     then "SFProFonts.pkg" > "Payload~" > "." > "Library" > "Fonts".
  c. Select all the "SF-Pro-Display-..." files, right-click > Install.
  d. Restart Wisp.


2. RUN IT
---------
  - Double-click wisp.exe. Nothing gets installed.
  - Windows may show "Windows protected your PC" because the app isn't
    signed. Click "More info" > "Run anyway".
  - The notch appears at the top center of your main screen.


3. TRY THESE
------------
  - Hover the notch to open it. Move away to close it.
  - Scroll or swipe on the open notch to switch pages:
    Now Playing, Calendar, To-do, Pomodoro, Shelf, Settings.
  - Calendar: pick a date and click the field on the left to add an
    event, e.g. "09:30-10:00 Standup" (just a title means 09:00). Click
    an event to edit it, or its x to delete it.
  - Pomodoro: between sessions, use the - / + buttons to change the length
    of focus, short and long breaks. Your lengths are saved.
  - Play music in Spotify, Apple Music or Spotifast, or a video in
    Chrome or Edge. The notch shows the track, and the lyrics button
    (bottom left) shows synced lyrics.
  - The volume slider moves the player's own volume slider (for Chrome
    and Edge, that browser's volume). Your Windows volume stays as it is.
  - Drop files on the notch to keep them on the shelf. Drag them back out.
  - Notifications from any app pop up in the notch.
  - When the notch would hide something (say, browser tabs that reach
    it), it shrinks to a thin sliver so it stays visible. An empty title
    bar behind the notch doesn't count. Move the mouse to
    the top edge to open it. Turn this off in Settings > "Shrink over
    content".
  - If two music apps are open, Wisp follows the one Windows shows in
    its media overlay (the one you used last). To pin one, use
    Settings > Music player.
  - Settings: click the gear at the right end of the notch's header, or click
    the Wisp icon in the system tray (near the clock, maybe behind the ^
    arrow) and choose "Settings...". "Quit Wisp" is in both places.


4. GOOD TO KNOW
---------------
  - Your to-dos, calendar events, shelf, settings and cached lyrics are
    stored in
    %APPDATA%\Wisp. Delete that folder to reset everything.
  - The only internet access is looking up lyrics on lrclib.net
    (it sends the song title, artist, album and length). If LRCLIB
    has no synced lyrics, it asks music.163.com (NetEase) with the
    title and artist.
  - Notifications are only read on your PC, never stored or sent.
  - To remove Wisp: quit it from the tray, delete wisp.exe and the
    %APPDATA%\Wisp folder. If you turned on "Start with Windows", turn it
    off first.


5. CREDITS
----------
Inter font: Copyright (c) 2016 The Inter Project Authors, SIL Open Font
License 1.1 (see Inter-LICENSE.txt). https://rsms.me/inter/


6. FEEDBACK
-----------
Please note: your Windows version, screen size and scaling, what you did,
what happened, and a screenshot if you can. Performance notes are welcome
too (Task Manager > Details > wisp.exe).
