"""Report whether the loaded WebKitGTK exposes WebRTC.

Deliberately independent of Tauricord: a bare WebKit2 WebView with the media
settings enabled. If RTCPeerConnection is undefined here, no amount of
application-level work will make Discord voice function.

    python3 scripts/webkit-probe.py
    LD_LIBRARY_PATH=~/.local/opt/webkit-webrtc/lib python3 scripts/webkit-probe.py
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
s.set_enable_media_capabilities(True)
wv.set_settings(s)

print("webkit build: %s.%s.%s" % (
    WebKit2.get_major_version(),
    WebKit2.get_minor_version(),
    WebKit2.get_micro_version(),
))
print("settings readback: media_stream=%s webrtc=%s" % (
    s.get_enable_media_stream(), s.get_enable_webrtc()))

win.add(wv)
win.show_all()

SCRIPT = """(function () {
  var out = [];
  out.push('RTCPeerConnection=' + (typeof RTCPeerConnection));
  out.push('RTCRtpSender=' + (typeof RTCRtpSender));
  out.push('RTCDataChannel=' + (typeof RTCDataChannel));
  out.push('mediaDevices=' + (typeof navigator.mediaDevices));
  out.push('isSecureContext=' + window.isSecureContext);
  try {
    var caps = RTCRtpSender.getCapabilities('audio');
    out.push('opus=' + caps.codecs.some(function (c) {
      return /opus/i.test(c.mimeType);
    }));
  } catch (e) {
    out.push('opus=ERR:' + e.message);
  }
  return out.join(' ');
})()"""


def on_result(webview, result, _data):
    try:
        print("RESULT:", webview.evaluate_javascript_finish(result).to_string())
    except Exception as e:  # noqa: BLE001
        print("EVAL ERROR:", e)
    Gtk.main_quit()


def on_load(webview, event):
    if event == WebKit2.LoadEvent.FINISHED:
        webview.evaluate_javascript(SCRIPT, -1, None, None, None, on_result, None)


wv.connect("load-changed", on_load)
# An https base URI guarantees a secure context, ruling that out as a cause.
wv.load_html("<html><body>probe</body></html>", "https://discord.com/")

GLib.timeout_add_seconds(20, Gtk.main_quit)
Gtk.main()
