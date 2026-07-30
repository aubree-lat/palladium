(function () {
  "use strict";

  if (window.__PALLADIUM_PROXY__) return;

  var BASE = "__PALLADIUM_PROXY_BASE__";
  window.__PALLADIUM_PROXY__ = BASE;

  var SAME = /(^|\.)(discord\.com|discordapp\.com|discordapp\.net|discord\.gg|discord\.media|discord\.tools)$/i;

  function targetUrl(input) {
    try {
      if (typeof input === "string") return new URL(input, location.href);
      if (input && input.url) return new URL(input.url, location.href);
    } catch (e) {}
    return null;
  }

  function eligible(url) {
    if (!url) return false;
    if (url.protocol !== "http:" && url.protocol !== "https:") return false;
    if (url.origin === location.origin) return false;
    if (SAME.test(url.hostname)) return false;
    if (url.hostname === "127.0.0.1" || url.hostname === "localhost") return false;
    return true;
  }

  function wrap(url) {
    return BASE + "?url=" + encodeURIComponent(url.href);
  }

  var nativeFetch = window.fetch;

  function report(msg) {
    try {
      nativeFetch(BASE + "/log?m=" + encodeURIComponent(msg), { cache: "no-store" });
    } catch (e) {}
  }
  window.__PALLADIUM_REPORT__ = report;

  window.fetch = function (input, init) {
    var url = targetUrl(input);
    return nativeFetch.call(window, input, init).catch(function (err) {
      if (!eligible(url)) {
        if (url && url.origin !== location.origin) {
          report("fetch failed, NOT eligible: " + url.href + " (" + err + ")");
        }
        throw err;
      }
      report("fetch blocked, retrying via endpoint: " + url.href);
      var opts = Object.assign({}, init || {});
      if (input && typeof input !== "string") {
        if (!opts.method) opts.method = input.method;
        if (!opts.headers) opts.headers = input.headers;
        if (!opts.body && input.body) opts.body = input.body;
      }
      delete opts.mode;
      delete opts.credentials;
      delete opts.integrity;
      return nativeFetch.call(window, wrap(url), opts);
    });
  };

  window.addEventListener(
    "keydown",
    function (e) {
      if (!e.ctrlKey || e.altKey || e.metaKey) return;
      var step = null;
      if (e.key === "=" || e.key === "+") step = 0.1;
      else if (e.key === "-" || e.key === "_") step = -0.1;
      else if (e.key === "0") step = 0;
      if (step === null) return;
      e.preventDefault();
      nativeFetch(BASE + "/zoom?step=" + step, { cache: "no-store" }).catch(
        function () {}
      );
    },
    true
  );

  var imgCache = new Map();

  function rescueImage(el) {
    var raw = el.getAttribute("src");
    if (!raw || el.dataset.pdRescued) return;
    var url = targetUrl(raw);
    if (!eligible(url)) return;

    el.dataset.pdRescued = "1";
    var key = url.href;

    if (imgCache.has(key)) {
      el.src = imgCache.get(key);
      return;
    }

    nativeFetch(wrap(url), { cache: "force-cache" })
      .then(function (res) {
        if (!res.ok) return null;
        return res.blob();
      })
      .then(function (blob) {
        if (!blob || !blob.size) return;
        var objectUrl = URL.createObjectURL(blob);
        imgCache.set(key, objectUrl);
        el.src = objectUrl;
      })
      .catch(function () {});
  }

  document.addEventListener(
    "error",
    function (e) {
      var el = e.target;
      if (el && el.tagName === "IMG") rescueImage(el);
    },
    true
  );

  var pasting = false;

  document.addEventListener(
    "paste",
    function (e) {
      if (pasting) return;

      var d = e.clipboardData;
      var hasFile = false;
      if (d) {
        if (d.files && d.files.length) hasFile = true;
        if (d.items) {
          for (var i = 0; i < d.items.length; i++) {
            if (d.items[i].kind === "file") hasFile = true;
          }
        }
        for (var j = 0; j < (d.types || []).length; j++) {
          if (String(d.types[j]).indexOf("image/") === 0) hasFile = true;
        }
      }
      if (hasFile) return;

      var target = e.target;

      nativeFetch(BASE + "/clipboard-image", { cache: "no-store" })
        .then(function (res) {
          if (res.status !== 200) return null;
          return res.blob();
        })
        .then(function (blob) {
          if (!blob || !blob.size) return;

          var file = new File([blob], "clipboard.png", { type: "image/png" });
          var dt = new DataTransfer();
          dt.items.add(file);

          pasting = true;
          try {
            var ev = new ClipboardEvent("paste", {
              clipboardData: dt,
              bubbles: true,
              cancelable: true,
            });
            var accepted = false;
            if (ev.clipboardData && ev.clipboardData.files.length) {
              accepted = (target || document.body).dispatchEvent(ev);
            }
            if (!ev.clipboardData || !ev.clipboardData.files.length) {
              var input = document.querySelector(
                'input[type="file"]:not([webkitdirectory])'
              );
              if (input) {
                input.files = dt.files;
                input.dispatchEvent(new Event("change", { bubbles: true }));
              } else {
                console.warn("[palladium] no paste target for clipboard image");
              }
            }
          } catch (err) {
            console.warn("[palladium] image paste failed", err);
          } finally {
            pasting = false;
          }
        })
        .catch(function () {});
    },
    true
  );

  var open = XMLHttpRequest.prototype.open;
  var send = XMLHttpRequest.prototype.send;

  XMLHttpRequest.prototype.open = function (method, url) {
    this.__tcMethod = method;
    this.__tcUrl = url;
    this.__tcArgs = Array.prototype.slice.call(arguments);
    return open.apply(this, arguments);
  };

  XMLHttpRequest.prototype.send = function (body) {
    var xhr = this;
    var url = targetUrl(xhr.__tcUrl);

    if (eligible(url) && !xhr.__tcRetried) {
      var onError = function () {
        xhr.removeEventListener("error", onError);
        xhr.__tcRetried = true;
        try {
          var args = xhr.__tcArgs.slice();
          args[1] = wrap(url);
          open.apply(xhr, args);
          send.call(xhr, body);
        } catch (e) {
          console.warn("[palladium] proxy retry failed", e);
        }
      };
      xhr.addEventListener("error", onError);
    }

    return send.apply(this, arguments);
  };
})();
