"""Does this WebKitGTK build actually gather ICE candidates?

Independent of Discord: builds a plain RTCPeerConnection with a public STUN
server, creates an offer, and reports every candidate it gathers. If this comes
back with zero candidates, no WebRTC call can ever connect regardless of the
application, because there is nothing to send connectivity checks from.

    LD_LIBRARY_PATH=~/.local/opt/webkit-webrtc/lib \
    GI_TYPELIB_PATH=~/.local/opt/webkit-webrtc/lib/girepository-1.0 \
    GDK_BACKEND=x11 python3 scripts/ice-probe.py
"""
import gi

gi.require_version("Gtk", "3.0")
gi.require_version("WebKit2", "4.1")
from gi.repository import GLib, Gtk, WebKit2  # noqa: E402

win = Gtk.OffscreenWindow()
wv = WebKit2.WebView()

s = wv.get_settings()
s.set_enable_media_stream(True)
s.set_enable_webrtc(True)
s.set_enable_mediasource(True)
wv.set_settings(s)

win.add(wv)
win.show_all()

SCRIPT = """
(function () {
  var resolve = function (msg) { document.title = "ICE " + msg; };
  var out = [];
  if (typeof RTCPeerConnection === "undefined") {
    resolve("NO_RTCPEERCONNECTION");
    return;
  }
  var pc;
  try {
    pc = new RTCPeerConnection({
      iceServers: [{ urls: "stun:stun.l.google.com:19302" }]
    });
  } catch (e) {
    resolve("CONSTRUCT_FAILED: " + e);
    return;
  }

  pc.onicecandidate = function (e) {
    if (e.candidate && e.candidate.candidate) {
      out.push(e.candidate.candidate);
    }
  };

  var done = function (tag) {
    var types = out.map(function (c) {
      var m = c.match(/typ (\\w+)/);
      return m ? m[1] : "?";
    });
    var counts = {};
    types.forEach(function (t) { counts[t] = (counts[t] || 0) + 1; });
    resolve(tag + " total=" + out.length + " " + JSON.stringify(counts) +
            " first=" + (out[0] || "none"));
  };

  pc.onicegatheringstatechange = function () {
    if (pc.iceGatheringState === "complete") done("GATHER_COMPLETE");
  };

  pc.createDataChannel("probe");
  pc.createOffer()
    .then(function (o) { return pc.setLocalDescription(o); })
    .catch(function (e) { resolve("OFFER_FAILED: " + e); });

  setTimeout(function () { done("TIMEOUT"); }, 12000);
})();
"""


def poll_title():
    title = wv.get_title() or ""
    if title.startswith("ICE "):
        print("RESULT:", title[4:])
        Gtk.main_quit()
        return False
    return True


def on_load(webview, event):
    if event == WebKit2.LoadEvent.FINISHED:
        webview.evaluate_javascript(SCRIPT, -1, None, None, None, None, None)
        GLib.timeout_add(500, poll_title)


wv.connect("load-changed", on_load)
wv.load_html("<html><body>ice probe</body></html>", "https://discord.com/")

GLib.timeout_add_seconds(30, Gtk.main_quit)
Gtk.main()
