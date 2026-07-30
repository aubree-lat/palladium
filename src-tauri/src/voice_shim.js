(function () {
  "use strict";

  if (window.__PALLADIUM_VOICE__) return;
  window.__PALLADIUM_VOICE__ = true;

  if (typeof RTCRtpSender === "undefined" || !RTCRtpSender.prototype.setParameters) {
    return;
  }

  var STRIPPABLE = [
    "maxBitrate",
    "scaleResolutionDownBy",
    "maxFramerate",
    "priority",
    "networkPriority",
    "adaptivePtime",
    "ptime",
  ];

  var nativeSetParameters = RTCRtpSender.prototype.setParameters;

  function stripped(params) {
    var out;
    try {
      out = JSON.parse(JSON.stringify(params || {}));
    } catch (e) {
      return null;
    }
    if (!out.encodings || !out.encodings.length) return out;
    out.encodings = out.encodings.map(function (enc) {
      STRIPPABLE.forEach(function (key) {
        delete enc[key];
      });
      return enc;
    });
    return out;
  }

  function unsupported(err) {
    var name = String((err && err.name) || "");
    var msg = String((err && err.message) || "");
    return /NotSupported/i.test(name) || /not supported/i.test(msg);
  }

  RTCRtpSender.prototype.setParameters = function (params) {
    var sender = this;
    return nativeSetParameters.call(sender, params).catch(function (err) {
      if (!unsupported(err)) throw err;

      var fallback = stripped(params);
      if (!fallback) return undefined;

      return nativeSetParameters.call(sender, fallback).catch(function () {
        return undefined;
      });
    });
  };

  if (window.__PALLADIUM_REPORT__) {
    window.__PALLADIUM_REPORT__("voice shim installed (setParameters tolerant)");
  }
})();
