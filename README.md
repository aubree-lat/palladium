# palladium, the rust based client for discord
hi, this is palladium, a rust based client thats in heavy work in progress rn, but pull requests are available and will do WONDERS to help me out with this slop
### it's still a HEAVY wip, and it could be VERY unstable
its literally just me
help
anyways
### you can ask me for more info on my discord server
### discord.gg/aubree

the whole thing is about 6mb. equibop on my machine is 314mb, and 103mb of that is a
bundled node runtime just to run arRPC, which palladium does in rust instead

## whats currently working so far
- basic functionality (discord launches)
- ~~voice chat (using a custom Webkit with WebRTC enabled)~~ (it broke again)
- wayland/x11 support
- equicord/vencord
- arRPC functionality
- image pasting
- importing your settings from vesktop/equibop (plugins, themes, quickcss)
- themes (oxygen, hydrogen, plutonium, uranium, iron, gallium) that can also restyle discord itself
- file uploads, notifications, spellcheck, ctrl +/- zoom
- windows builds, cross compiled from linux

## whats NOT working
- voice chat, it gets stuck on "checking route" and reconnects forever.
  its an ICE bug in webkit, not really fixable from inside the app
- reviewdb and anything else that calls a third party api, webkit blocks those
- screenshare (untested, probably broken)

## running it
you need rust, node, and the tauri deps (webkit2gtk-4.1, libsoup3, gtk3)

```bash
npm install
npm run dev
```

thats it for normal use

if you want to try the voice stuff anyway:

```bash
npm run setup:librice   # builds the ICE backend
npm run setup:webrtc    # builds webkit with webrtc, this takes 1-3 HOURS
npm run start:webrtc
```

fair warning that voice still doesnt connect even after all that, so probably dont

## for windows
```bash
npm run setup:windows
npm run build:windows
```

## want to help?
theres a big writeup of how everything actually works in [docs/TECHNICAL.md](docs/TECHNICAL.md),
including all the webkit workarounds and why voice is broken

## credits
- [vencord](https://github.com/Vendicated/Vencord) and [equicord](https://github.com/Equicord/Equicord)
- [arRPC](https://github.com/OpenAsar/arrpc) by openasar, palladium reimplements their protocol in rust
- [vesktop](https://github.com/Vencord/Vesktop) and [equibop](https://github.com/Equicord/Equibop), use these if you actually need voice
